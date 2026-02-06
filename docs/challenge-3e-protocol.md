# Challenge 3e: Efficient Broadcast Part II - Protocol Proposal

## Challenge Requirements

- **Nodes:** 25
- **Message delay:** 100ms
- **Messages-per-operation:** < 20
- **Median latency:** < 1 second
- **Maximum latency:** < 2 seconds

## Current Problem

With naive gossip (flooding), each broadcast fans out to all neighbors, causing:
- Duplicate messages across the network
- O(n²) message complexity
- Wasted bandwidth and higher latency

## Proposed Protocol: Batched Tree-Based Gossip

### 1. Topology: Spanning Tree with Shortcuts

Instead of a grid or full mesh, build a **balanced spanning tree** with depth ≤ 3:

```
         n0 (root)
       /    \
     n1      n2
    / \     / \
  n3  n4  n5  n6
  ...
```

**Benefits:**
- Any node reaches any other in ≤ 2 hops (through root)
- Minimal message duplication
- Predictable latency

**Implementation:**
- On `topology` message, compute a tree rooted at `n0`
- Each node stores only its parent and children
- Optionally add 1-2 "shortcut" edges for fault tolerance

### 2. Batching: Aggregate Messages Before Sending

Instead of sending a gossip message per broadcast, **batch multiple values**:

```
Batch Window: 50-100ms
```

**Flow:**
1. On `broadcast`, store value locally and mark "pending gossip"
2. Every 50ms (or when batch reaches N items), send batched gossip to neighbors
3. Include all unsent values in one message

**Message format:**
```json
{
  "type": "gossip",
  "values": [1, 2, 3, 4, 5]
}
```

**Benefits:**
- Reduces messages-per-operation significantly
- Amortizes network delay across multiple values

### 3. Delta Gossip: Only Send What's New

Track what each neighbor has seen using **version vectors** or **bloom filters**:

```rust
struct NodeState {
    seen: HashSet<u64>,
    neighbor_seen: HashMap<String, HashSet<u64>>,
}
```

**Flow:**
1. On gossip, compute `delta = my_seen - neighbor_seen[peer]`
2. Send only the delta
3. On gossip_ok, update `neighbor_seen[peer]`

**Benefits:**
- Avoids resending already-known values
- Reduces payload size over time

### 4. Anti-Entropy: Periodic Full Sync

For partition tolerance, run periodic anti-entropy:

```
Every 500ms:
  - Pick random neighbor
  - Exchange full state (or bloom filter)
  - Reconcile differences
```

**Benefits:**
- Recovers from message loss
- Handles network partitions gracefully

## Message Flow Summary

```
Client → Node: broadcast(value)
  └─ Node stores value, acks immediately
  └─ Batches value for gossip

Every 50ms:
  Node → Children/Parent: gossip(values=[...])
    └─ Recipients merge values, ack with gossip_ok

Client → Node: read
  └─ Node returns all seen values
```

## Estimated Performance

| Metric | Target | Expected |
|--------|--------|----------|
| Messages-per-op | < 20 | ~10-15 (batching + tree) |
| Median latency | < 1s | ~300-500ms (2 hops × 100ms + batch delay) |
| Max latency | < 2s | ~800ms-1.5s (partition recovery) |

## Implementation Steps

1. **Modify `topology`:** Build spanning tree, store parent/children
2. **Add batch buffer:** Accumulate broadcasts before gossip
3. **Add batch timer:** Flush every 50ms or on threshold
4. **Track neighbor state:** Delta gossip to reduce redundancy
5. **Add anti-entropy:** Periodic sync for fault tolerance

## Trade-offs

| Approach | Pros | Cons |
|----------|------|------|
| Tree topology | Low message count | Single point of failure (root) |
| Batching | Fewer messages | Adds latency (batch window) |
| Delta gossip | Smaller payloads | Memory for tracking state |
| Anti-entropy | Partition tolerance | Extra background messages |

## Conclusion

Combining **tree topology**, **batching**, and **delta gossip** should achieve < 20 messages-per-operation while keeping latency under 1 second median. Anti-entropy ensures correctness under partitions.
