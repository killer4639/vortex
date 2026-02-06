use std::{collections::HashMap, str::FromStr};

use crate::challenges::{
    cluster::global_cluster,
    node::{GcounterData, Node},
};

use super::super::{BodyBase, Message, send};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::Write;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InitBody {
    #[serde(flatten)]
    pub base: BodyBase,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_ids: Option<Vec<String>>,
}

/// Replies to an init message with init_ok.
pub fn init(msg: Message<InitBody>, output: &mut impl Write) -> Result<()> {
    let node_id = msg.body.node_id.clone().unwrap();
    let peers = msg.body.node_ids.clone().unwrap();
    let node: Node = Node {
        id: node_id.clone(),
        peers,
        next_msg_id: 0,
        gcounter_data: GcounterData {
            node_data: HashMap::new(),
            gossip_thread: None
        },
    };

    let cluster = global_cluster();
    let mut cluster = cluster.write().expect("cluster lock poisoned");
    cluster.add_node(node);

    let response: Message<InitBody> = Message {
        src: node_id,
        dest: msg.src,
        body: InitBody {
            base: BodyBase {
                typ: String::from_str("init_ok").unwrap(),
                in_reply_to: msg.body.base.msg_id,
                msg_id: None,
            },
            node_id: None,
            node_ids: None,
        },
    };

    send(&response, output)
}
