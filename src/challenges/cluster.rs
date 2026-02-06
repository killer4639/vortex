use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use super::node::Node;

pub struct Cluster {
    pub nodes: HashMap<String, Node>,
    pub is_topology_done: bool
}

impl Cluster {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            is_topology_done: false
        }
    }

    pub fn add_node(&mut self, node: Node) {
        let id = node.id.clone();
        self.nodes.insert(id, node);
    }

    pub fn get_node_mut(&mut self, id: &str) -> Option<&mut Node> {
        self.nodes.get_mut(id)
    }
}

static CLUSTER: OnceLock<RwLock<Cluster>> = OnceLock::new();

pub fn global_cluster() -> &'static RwLock<Cluster> {
    CLUSTER.get_or_init(|| RwLock::new(Cluster::new()))
}
