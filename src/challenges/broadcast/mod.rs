pub mod gossip;

use crate::{
    send, BodyBase, Message,
    challenges::{broadcast::gossip::GossipBody, cluster::global_cluster},
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Write;

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

#[derive(Debug, Clone)]
pub struct BroadcastData {
    pub data: HashSet<u64>,
}

impl BroadcastData {
    pub fn add_data(&mut self, data: u64) -> () {
        self.data.insert(data);
    }

    pub fn merge_sets(&mut self, data: HashSet<u64>) {
        self.data.extend(data);
    }
}

pub fn broadcast(msg: Message<BroadcastBody>, output: &mut impl Write) -> Result<()> {
    let mut cluster = global_cluster().write().unwrap();
    let node = cluster.get_node_mut(&msg.dest).unwrap();
    if node.broadcast_data.is_none() {
        node.broadcast_data = Some(BroadcastData {
            data: HashSet::new(),
        })
    }
    {
        let broadcast_data = node.broadcast_data.as_mut().unwrap();
        broadcast_data.add_data(msg.body.message.unwrap());
    }

    let peers = node.peers.clone();
    let src = node.id.clone();
    let gossip_data = node.broadcast_data.clone().unwrap().data;
    let mut gossip_messages: Vec<Message<GossipBody>> = Vec::new();
    for peer in peers {
        if peer == node.id {
            continue;
        }
        let gossip_message: Message<_> = Message {
            src: src.clone(),
            dest: peer,
            body: GossipBody {
                base: BodyBase {
                    typ: "gossip".to_string(),
                    msg_id: Some(node.get_next_id()),
                    in_reply_to: None,
                },
                gossip_data: Some(gossip_data.clone()),
            },
        };
        gossip_messages.push(gossip_message);
    }

    let response: Message<BroadcastBody> = Message {
        src: node.id.clone(),
        dest: msg.src,
        body: BroadcastBody {
            base: BodyBase {
                typ: "broadcast_ok".to_string(),
                in_reply_to: msg.body.base.msg_id,
                msg_id: Some(node.get_next_id()),
            },
            message: None,
        },
    };
    drop(cluster);

    for gossip_message in gossip_messages {
        send(&gossip_message, output)?;
    }
    send(&response, output)
}

pub fn read(msg: Message<ReadBody>, output: &mut impl Write) -> Result<()> {
    let mut cluster = global_cluster().write().unwrap();
    let node = cluster.get_node_mut(&msg.dest).unwrap();
    if node.broadcast_data.is_none() {
        node.broadcast_data = Some(BroadcastData {
            data: HashSet::new(),
        })
    }
    let messages = {
        let broadcast_data = node.broadcast_data.as_ref().unwrap();
        broadcast_data.data.clone()
    };
    let msg_id = node.get_next_id();
    let src = node.id.clone();

    let response: Message<ReadBody> = Message {
        src,
        dest: msg.src,
        body: ReadBody {
            base: BodyBase {
                typ: "read_ok".to_string(),
                in_reply_to: msg.body.base.msg_id,
                msg_id: Some(msg_id),
            },
            messages: Some(messages),
        },
    };
    drop(cluster);
    send(&response, output)
}

pub fn topology(msg: Message<TopologyBody>, output: &mut impl Write) -> Result<()> {
    let mut cluster = global_cluster().write().unwrap();
    let node = cluster.get_node_mut(&msg.dest).unwrap();

    let response: Message<TopologyBody> = Message {
        src: node.id.clone(),
        dest: msg.src,
        body: TopologyBody {
            base: BodyBase {
                typ: "topology_ok".to_string(),
                in_reply_to: msg.body.base.msg_id,
                msg_id: Some(node.get_next_id()),
            },
            topology: None,
        },
    };
    drop(cluster);
    send(&response, output)
}
