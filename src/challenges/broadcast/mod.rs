pub mod gossip;
pub mod lru_cache;

use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::Write,
    thread,
    time::Duration,
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    BodyBase, Message,
    challenges::{broadcast::gossip::GossipBody, cluster::global_cluster},
    send,
};

// ============================================================================
// Message Body Types
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BroadcastBody {
    #[serde(flatten)]
    pub base: BodyBase,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReadBody {
    #[serde(flatten)]
    pub base: BodyBase,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<HashSet<u64>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TopologyBody {
    #[serde(flatten)]
    pub base: BodyBase,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology: Option<HashMap<String, Vec<String>>>,
}

// ============================================================================
// Broadcast Data Store
// ============================================================================

#[derive(Debug, Clone, Default)]
pub struct BroadcastData {
    pub data: HashSet<u64>,
    pub seen_msg: HashSet<(String, u64)>,
    pub last_gossip_len: usize,
}

impl BroadcastData {
    pub fn new() -> Self {
        Self {
            data: HashSet::new(),
            seen_msg: HashSet::new(),
            last_gossip_len: 0,
        }
    }

    pub fn insert(&mut self, value: u64) {
        self.data.insert(value);
    }

    pub fn extend(&mut self, values: HashSet<u64>) {
        self.data.extend(values);
    }

    pub fn clone_data(&self) -> HashSet<u64> {
        self.data.clone()
    }

    pub fn add_if_not_present(&mut self, origin: &str, msg_id: u64) -> bool {
        let key = (origin.to_string(), msg_id);
        if self.seen_msg.contains(&key) {
            false
        } else {
            self.seen_msg.insert(key);
            true
        }
    }
}

// ============================================================================
// Gossip Thread
// ============================================================================

const GOSSIP_INTERVAL_MS: u64 = 50;

fn spawn_gossip_thread(node_id: String) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(GOSSIP_INTERVAL_MS));

            let Some((src, data, peers)) = prepare_gossip_batch(&node_id) else {
                continue;
            };

            if peers.is_empty() {
                continue;
            }

            send_gossip_to_peers(&src, &data, &peers, rand::random::<u64>(), &src);
        }
    })
}

pub fn prepare_gossip_batch(node_id: &str) -> Option<(String, HashSet<u64>, Vec<(String, u64)>)> {
    let mut cluster = global_cluster().write().unwrap();
    let node = cluster.get_node_mut(node_id)?;

    let broadcast_data = node.broadcast_data.get_or_insert_with(BroadcastData::new);
    let gossip_data = broadcast_data.clone_data();
    if gossip_data.len() == broadcast_data.last_gossip_len {
        return None;
    }
    broadcast_data.last_gossip_len = gossip_data.len();
    let src = node.id.clone();
    let node_id_owned = node.id.clone();

    // Clone peer list to avoid borrow conflicts
    let peer_list: Vec<String> = node.peers.clone();

    // Generate message IDs for each peer (excluding self)
    let peers: Vec<(String, u64)> = peer_list
        .into_iter()
        .filter(|peer| peer != &node_id_owned)
        .map(|peer| {
            let msg_id = node.get_next_id();
            (peer, msg_id)
        })
        .collect();

    Some((src, gossip_data, peers))
}

pub fn send_gossip_to_peers(
    src: &str,
    data: &HashSet<u64>,
    peers: &[(String, u64)],
    org_msg_id: u64,
    org_msg_src: &str,
) {
    let mut stdout = std::io::stdout().lock();

    for (peer, msg_id) in peers {
        let message = create_gossip_message(
            src,
            peer,
            *msg_id,
            data.clone(),
            org_msg_id,
            org_msg_src,
        );
        let _ = send(&message, &mut stdout);
    }
}

fn create_gossip_message(
    src: &str,
    dest: &str,
    msg_id: u64,
    data: HashSet<u64>,
    org_msg_id: u64,
    org_msg_src: &str,
) -> Message<GossipBody> {
    Message {
        src: src.to_string(),
        dest: dest.to_string(),
        body: GossipBody {
            base: BodyBase {
                typ: "gossip".to_string(),
                msg_id: Some(msg_id),
                in_reply_to: None,
            },
            gossip_data: Some(data),
            org_msg_id,
            org_msg_src: org_msg_src.to_string(),
        },
    }
}

// ============================================================================
// Message Handlers
// ============================================================================

pub fn broadcast(msg: Message<BroadcastBody>, output: &mut impl Write) -> Result<()> {
    let (response, gossip_messages) = {
        let mut cluster = global_cluster().write().unwrap();
        let node = cluster.get_node_mut(&msg.dest).unwrap();

        // Initialize broadcast data if needed
        let broadcast_data = node.broadcast_data.get_or_insert_with(BroadcastData::new);

        // Store the incoming message
        if let Some(value) = msg.body.message {
            broadcast_data.insert(value);
        }

        // Spawn gossip thread on first broadcast
        if node.gossip_thread.is_none() {
            let handle = spawn_gossip_thread(node.id.clone());
            node.gossip_thread = Some(handle.thread().clone());
        }

        // Prepare gossip messages for all peers
        let gossip_data = broadcast_data.clone_data();
        broadcast_data.last_gossip_len = gossip_data.len();
        let node_id = node.id.clone();

        let peer_list: Vec<String> = node
            .peers
            .iter()
            .filter(|peer| *peer != &node_id)
            .cloned()
            .collect();

        let gossip_messages: Vec<_> = peer_list
            .into_iter()
            .map(|peer| {
                let msg_id = node.get_next_id();
                create_gossip_message(
                    &node_id,
                    &peer,
                    msg_id,
                    gossip_data.clone(),
                    msg.body.base.msg_id.unwrap(),
                    &msg.src,
                )
            })
            .collect();

        // Build response
        let response = Message {
            src: node.id.clone(),
            dest: msg.src.clone(),
            body: BroadcastBody {
                base: BodyBase {
                    typ: "broadcast_ok".to_string(),
                    msg_id: Some(node.get_next_id()),
                    in_reply_to: msg.body.base.msg_id,
                },
                message: None,
            },
        };

        (response, gossip_messages)
    };

    // Send all messages outside the lock
    for gossip_msg in gossip_messages {
        send(&gossip_msg, output)?;
    }
    send(&response, output)
}

pub fn read(msg: Message<ReadBody>, output: &mut impl Write) -> Result<()> {
    let response = {
        let mut cluster = global_cluster().write().unwrap();
        let node = cluster.get_node_mut(&msg.dest).unwrap();

        let broadcast_data = node.broadcast_data.get_or_insert_with(BroadcastData::new);
        let messages = broadcast_data.clone_data();

        Message {
            src: node.id.clone(),
            dest: msg.src.clone(),
            body: ReadBody {
                base: BodyBase {
                    typ: "read_ok".to_string(),
                    msg_id: Some(node.get_next_id()),
                    in_reply_to: msg.body.base.msg_id,
                },
                messages: Some(messages),
            },
        }
    };

    send(&response, output)
}

pub fn topology(msg: Message<TopologyBody>, output: &mut impl Write) -> Result<()> {
    let response = {
        let mut cluster = global_cluster().write().unwrap();
        let node = cluster.get_node_mut(&msg.dest).unwrap();
        let node_id = node.id.clone();
        let all_nodes = node.peers.clone();

        if !cluster.is_topology_done {
            let graph = build_optimized_topology(&all_nodes);
            apply_topology_to_cluster(&mut cluster, &graph, &all_nodes);
            cluster.is_topology_done = true;
        }

        Message {
            src: node_id,
            dest: msg.src.clone(),
            body: TopologyBody {
                base: BodyBase {
                    typ: "topology_ok".to_string(),
                    msg_id: None,
                    in_reply_to: msg.body.base.msg_id,
                },
                topology: None,
            },
        }
    };

    send(&response, output)
}

// ============================================================================
// Topology Building
// ============================================================================

/// Builds an optimized topology graph where all nodes are within 2 hops of each other.
fn build_optimized_topology(nodes: &[String]) -> HashMap<String, Vec<String>> {
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();

    // First pass: create a linear chain
    for window in nodes.windows(2) {
        let (a, b) = (&window[0], &window[1]);
        add_bidirectional_edge(&mut graph, a, b);
    }

    // Second pass: add shortcut edges for nodes more than 2 hops apart
    for i in 0..nodes.len() {
        for j in (i + 1)..nodes.len() {
            if !is_within_two_hops(&graph, &nodes[i], &nodes[j]) {
                add_bidirectional_edge(&mut graph, &nodes[i], &nodes[j]);
            }
        }
    }

    graph
}

fn add_bidirectional_edge(graph: &mut HashMap<String, Vec<String>>, a: &str, b: &str) {
    graph.entry(a.to_string()).or_default().push(b.to_string());
    graph.entry(b.to_string()).or_default().push(a.to_string());
}

fn apply_topology_to_cluster(
    cluster: &mut crate::challenges::cluster::Cluster,
    graph: &HashMap<String, Vec<String>>,
    nodes: &[String],
) {
    for node_id in nodes {
        if let Some(node) = cluster.get_node_mut(node_id) {
            node.peers = graph.get(node_id).cloned().unwrap_or_default();
        }
    }
}

/// Checks if two nodes are within 2 hops of each other using BFS.
fn is_within_two_hops(graph: &HashMap<String, Vec<String>>, start: &str, target: &str) -> bool {
    if start == target {
        return true;
    }

    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();

    visited.insert(start.to_string());
    queue.push_back((start.to_string(), 0));

    while let Some((current, depth)) = queue.pop_front() {
        if depth >= 2 {
            continue;
        }

        if let Some(neighbors) = graph.get(&current) {
            for neighbor in neighbors {
                if neighbor == target {
                    return true;
                }
                if visited.insert(neighbor.clone()) {
                    queue.push_back((neighbor.clone(), depth + 1));
                }
            }
        }
    }

    false
}
