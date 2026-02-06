use std::{
    io::Write,
    thread::{self, Thread},
    time::Duration,
};

use anyhow::{Ok, Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::{
    BodyBase, Message,
    challenges::{cluster::global_cluster},
    send,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReadBody {
    #[serde(flatten)]
    base: BodyBase,

    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AddBody {
    #[serde(flatten)]
    base: BodyBase,

    #[serde(skip_serializing_if = "Option::is_none")]
    delta: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GossipBody {
    #[serde(flatten)]
    base: BodyBase,

    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<u64>,
}

fn spawn_gossip_thread(node_id: String) -> Thread {
    let handle = thread::spawn(move || {
        let mut stdout = std::io::stdout();

        loop {
            thread::sleep(Duration::from_millis(100));

            let (current_value, targets) = {
                let mut cluster = global_cluster()
                    .write()
                    .map_err(|_| anyhow!("cluster lock poisoned"))
                    .unwrap();

                let Some(node) = cluster.get_node_mut(&node_id) else {
                    continue;
                };

                let peers = node.peers.clone();

                let current_value = node
                    .gcounter_data
                    .node_data
                    .get(&node_id)
                    .copied()
                    .unwrap_or(0);

                let mut targets = Vec::new();
                for peer in peers.iter() {
                    if peer == &node_id {
                        continue;
                    }

                    let msg_id = node.get_next_id();
                    targets.push((peer.clone(), msg_id));
                }

                (current_value, targets)
            };

            for (peer, msg_id) in targets {
                let msg = Message {
                    src: node_id.clone(),
                    dest: peer,
                    body: GossipBody {
                        base: BodyBase {
                            typ: "gossip".to_string(),
                            msg_id: Some(msg_id),
                            in_reply_to: None,
                        },
                        value: Some(current_value),
                    },
                };

                let _ = send(&msg, &mut stdout);
            }
        }
    });

    handle.thread().clone()
}

pub fn add(msg: Message<AddBody>, output: &mut impl Write) -> Result<()> {
    let mut cluster = global_cluster()
        .write()
        .map_err(|_| anyhow!("cluster lock poisoned"))?;
    let node = cluster
        .get_node_mut(&msg.dest)
        .ok_or_else(|| anyhow!("unknown node: {}", msg.dest))?;

    let node_id = node.id.clone();
    if node.gcounter_data.gossip_thread.is_none() {
        node.gcounter_data.gossip_thread = Some(spawn_gossip_thread(node_id));
    }

    let delta = msg
        .body
        .delta
        .ok_or_else(|| anyhow!("missing delta in add request"))?;

    node.gcounter_data
        .node_data
        .entry(node.id.clone())
        .and_modify(|value| *value += delta)
        .or_insert(delta);

    let response = Message {
        src: node.id.clone(),
        dest: msg.src,
        body: AddBody {
            base: BodyBase {
                typ: "add_ok".to_string(),
                msg_id: Some(node.get_next_id()),
                in_reply_to: msg.body.base.msg_id,
            },
            delta: None,
        },
    };

    send(&response, output)
}
pub fn read(msg: Message<ReadBody>, output: &mut impl Write) -> Result<()> {
    let mut cluster = global_cluster()
        .write()
        .map_err(|_| anyhow!("cluster lock poisoned"))?;
    let node = cluster
        .get_node_mut(&msg.dest)
        .ok_or_else(|| anyhow!("unknown node: {}", msg.dest))?;

    let sum = node.gcounter_data.node_data.values().sum::<u64>();

    let response = Message {
        src: node.id.clone(),
        dest: msg.src,
        body: ReadBody {
            base: BodyBase {
                typ: "read_ok".to_string(),
                msg_id: Some(node.get_next_id()),
                in_reply_to: msg.body.base.msg_id,
            },
            value: Some(sum),
        },
    };

    send(&response, output)
}

pub fn gossip(msg: Message<GossipBody>, output: &mut impl Write) -> Result<()> {
    let mut cluster = global_cluster()
        .write()
        .map_err(|_| anyhow!("cluster lock poisoned"))?;
    let cur_node = cluster
        .get_node_mut(&msg.dest)
        .ok_or_else(|| anyhow!("unknown node: {}", msg.dest))?;

    cur_node
        .gcounter_data
        .node_data
        .insert(msg.src, msg.body.value.unwrap());
    Ok(())
}
