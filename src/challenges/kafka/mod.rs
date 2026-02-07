use anyhow::{Ok, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    io::Write,
};

use crate::{BodyBase, Message, challenges::cluster::global_cluster, send};

#[derive(Debug)]
pub struct KafkaNodeData {
    pub logs: HashMap<String, VecDeque<u64>>,
    pub offsets: HashMap<String, u64>,
    pub offsets_commited: HashMap<String, u64>,
}

impl KafkaNodeData {
    pub fn new() -> Self {
        Self {
            logs: HashMap::new(),
            offsets: HashMap::new(),
            offsets_commited: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SendBody {
    #[serde(flatten)]
    body: BodyBase,

    #[serde(skip_serializing_if = "Option::is_none")]
    key: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    msg: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    offset: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommitBody {
    #[serde(flatten)]
    body: BodyBase,

    #[serde(skip_serializing_if = "Option::is_none")]
    offsets: Option<HashMap<String, u64>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PollBody {
    #[serde(flatten)]
    body: BodyBase,

    #[serde(skip_serializing_if = "Option::is_none")]
    msgs: Option<HashMap<String, Vec<Vec<u64>>>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    offsets: Option<HashMap<String, u64>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListOffsetsBody {
    #[serde(flatten)]
    body: BodyBase,

    #[serde(skip_serializing_if = "Option::is_none")]
    keys: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    offsets: Option<HashMap<String, u64>>,
}

pub fn send_log(msg: Message<SendBody>, output: &mut impl Write) -> Result<()> {
    let mut cluster = global_cluster().write().unwrap();
    let node = cluster.get_node_mut(&msg.dest).unwrap();
    let key = msg.body.key.clone().unwrap();
    let current_offset = *node.kafka_data.offsets.get(&key).unwrap_or(&0);

    let new_offset = current_offset + 1;
    node.kafka_data
        .offsets
        .insert(key.clone(), current_offset + 1);

    if node.kafka_data.logs.contains_key(&key) {
        node.kafka_data
            .logs
            .get_mut(&key)
            .unwrap_or(&mut VecDeque::new())
            .push_back(msg.body.msg.unwrap());
    } else {
        let mut deque = VecDeque::new();
        deque.push_back(msg.body.msg.unwrap());
        node.kafka_data.logs.insert(key.clone(), deque);
    }

    let response = Message {
        src: node.id.clone(),
        dest: msg.src,
        body: SendBody {
            body: BodyBase {
                typ: "send_ok".to_string(),
                msg_id: Some(node.get_next_id()),
                in_reply_to: msg.body.body.msg_id,
            },
            key: None,
            msg: None,
            offset: Some(new_offset),
        },
    };

    send(&response, output)
}

pub fn poll(msg: Message<PollBody>, output: &mut impl Write) -> Result<()> {
    let mut cluster = global_cluster().write().unwrap();
    let node = cluster.get_node_mut(&msg.dest).unwrap();

    let mut msgs: HashMap<String, Vec<Vec<u64>>> = HashMap::new();
    let offsets = msg.body.offsets.unwrap_or_default();

    for (key, offset) in offsets.iter() {
        let committed = node
            .kafka_data
            .offsets_commited
            .get(key)
            .copied()
            .unwrap_or(0);
        let start = offset.saturating_sub(committed).saturating_sub(1) as usize;

        let mut data: Vec<Vec<u64>> = Vec::new();
        if let Some(logs) = node.kafka_data.logs.get(key) {
            for (idx, msg_value) in logs.iter().skip(start).enumerate() {
                let real_offset = committed as u64 + idx as u64 + 1;
                data.push(vec![real_offset, *msg_value]);
            }
        }
        msgs.insert(key.clone(), data);
    }

    let response = Message {
        src: node.id.clone(),
        dest: msg.src,
        body: PollBody {
            body: BodyBase {
                typ: "poll_ok".to_string(),
                msg_id: Some(node.get_next_id()),
                in_reply_to: msg.body.body.msg_id,
            },
            msgs: Some(msgs),
            offsets: None,
        },
    };

    send(&response, output)
}

pub fn commit(msg: Message<CommitBody>, output: &mut impl Write) -> Result<()> {
    let mut cluster = global_cluster().write().unwrap();
    let node = cluster.get_node_mut(&msg.dest).unwrap();

    let offsets = msg.body.offsets.unwrap_or_default();

    for (key, offset) in offsets.iter() {
        let committed = node
            .kafka_data
            .offsets_commited
            .get(key)
            .copied()
            .unwrap_or(0);

        let to_commit = offset - committed;

        for _counter in 0..to_commit {
            node.kafka_data.logs.get_mut(key).unwrap().pop_front();
        }

        node.kafka_data
            .offsets_commited
            .insert(key.clone(), *offset);
    }

    let response = Message {
        src: node.id.clone(),
        dest: msg.src,
        body: CommitBody {
            body: BodyBase {
                typ: "commit_offsets_ok".to_string(),
                msg_id: Some(node.get_next_id()),
                in_reply_to: msg.body.body.msg_id,
            },
            offsets: None,
        },
    };

    send(&response, output)
}

pub fn list_offset_bodies(msg: Message<ListOffsetsBody>, output: &mut impl Write) -> Result<()> {
    let mut cluster = global_cluster().write().unwrap();
    let node = cluster.get_node_mut(&msg.dest).unwrap();

    let mut offset_bodies: HashMap<String, u64> = HashMap::new();

    for key in msg.body.keys.unwrap().iter() {
        let committed = node
            .kafka_data
            .offsets_commited
            .get(key)
            .copied()
            .unwrap_or(0);

        offset_bodies.insert(key.clone(), committed);
    }

    let response = Message {
        src: node.id.clone(),
        dest: msg.src,
        body: ListOffsetsBody {
            body: BodyBase {
                typ: "list_committed_offsets_ok".to_string(),
                msg_id: Some(node.get_next_id()),
                in_reply_to: msg.body.body.msg_id,
            },
            offsets: Some(offset_bodies),
            keys: None,
        },
    };

    send(&response, output)
}
