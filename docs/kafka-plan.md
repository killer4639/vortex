
## Plan: Single-Node Kafka-Style Log (Challenge 5a)

This challenge requires implementing a single-node append-only log service handling 4 RPCs: `send`, `poll`, `commit_offsets`, and `list_committed_offsets`. Since it's single-node, no gossip or inter-node communication is needed. The implementation follows the existing pattern: define body structs, add `TypedMessage` variants, write handler functions, and wire them into the main dispatch loop.

**Steps**

1. **Create the kafka module** at `src/challenges/kafka/mod.rs` and register it in `src/challenges/mod.rs` with `pub mod kafka;`

2. **Add kafka state to `Node`** in `src/challenges/node.rs`. Add a new `KafkaData` struct:
   - `logs: HashMap<String, Vec<(u64, u64)>>` — maps key → list of `(offset, message)` pairs
   - `committed_offsets: HashMap<String, u64>` — maps key → last committed offset
   - `next_offset: HashMap<String, u64>` — maps key → next offset to assign (start at 0 or 1)
   
   Add a `kafka_data: KafkaData` field to `Node`, initialized with empty maps.

3. **Define request/response body structs** in `src/challenges/kafka/mod.rs`:
   - `SendBody` — `base: BodyBase`, `key: String`, `msg: u64`
   - `SendOkBody` — `base: BodyBase`, `offset: u64`
   - `PollBody` — `base: BodyBase`, `offsets: HashMap<String, u64>`
   - `PollOkBody` — `base: BodyBase`, `msgs: HashMap<String, Vec<[u64; 2]>>` (each entry is `[offset, msg]`)
   - `CommitOffsetsBody` — `base: BodyBase`, `offsets: HashMap<String, u64>`
   - `CommitOffsetsOkBody` — `base: BodyBase`
   - `ListCommittedOffsetsBody` — `base: BodyBase`, `keys: Vec<String>`
   - `ListCommittedOffsetsOkBody` — `base: BodyBase`, `offsets: HashMap<String, u64>`

4. **Implement 4 handler functions** in the kafka module:
   - `send()` — Look up the key's log, assign the next offset (monotonically increasing per key), append `(offset, msg)`, reply with `send_ok` containing the offset
   - `poll()` — For each key+offset in the request, find messages at or after that offset in the log, return them as `[[offset, msg], ...]` pairs in `poll_ok`
   - `commit_offsets()` — Store each key's committed offset in `committed_offsets`, reply with `commit_offsets_ok`
   - `list_committed_offsets()` — Look up committed offsets for the requested keys, omit keys that don't exist, reply with `list_committed_offsets_ok`

5. **Add `TypedMessage` variants** in `src/main.rs` — add `Send`, `Poll`, `CommitOffsets`, `ListCommittedOffsets` variants to the `TypedMessage` enum

6. **Add dispatch arms** in `parse_typed_message()` in `src/main.rs` — map `"send"`, `"poll"`, `"commit_offsets"`, `"list_committed_offsets"` strings to their respective variants

7. **Add match arms** in the `main()` loop in `src/main.rs` — call the corresponding handler functions from the kafka module

8. **Update the maelstrom test command** — the binary name is `vortex` (not `maelstrom-kafka`), so the test command will be:
   ```
   ./maelstrom test -w kafka --bin target/release/vortex --node-count 1 --concurrency 2n --time-limit 20 --rate 1000
   ```

**Key design decisions**

- **Offset strategy**: Use a simple per-key counter starting at 0, incrementing by 1 on each `send`. This guarantees monotonically increasing offsets and no gaps (satisfying the "no lost writes" invariant).
- **Poll implementation**: Since logs are stored in offset order, use binary search or linear scan from the requested offset. With a `Vec` in insertion order and monotonic offsets, a `partition_point` / `binary_search` is efficient.
- **Serialization of poll msgs**: The Maelstrom protocol expects `[[offset, msg], ...]` arrays, not objects. Use `Vec<(u64, u64)>` or `Vec<[u64; 2]>` which serde serializes as JSON arrays of arrays.
- **No concurrency concerns for 5a**: Single-node with the existing `RwLock<Cluster>` pattern is sufficient.

**Verification**

- Build with `cargo build --release`
- Run: `./maelstrom test -w kafka --bin target/release/vortex --node-count 1 --concurrency 2n --time-limit 20 --rate 1000`
- Maelstrom validates: no lost writes, monotonically increasing offsets, correct commit tracking

**Design choices**

- Chose per-key `next_offset` counter over a global offset counter — simpler and aligns with kafka partition semantics
- Chose `Vec<(u64, u64)>` for log storage over a `BTreeMap<u64, u64>` — insertion is always at the end (append-only), and polling by offset can use binary search on the sorted vec, which is cache-friendly and simple
- No background threads needed for 5a (single-node, no replication)
