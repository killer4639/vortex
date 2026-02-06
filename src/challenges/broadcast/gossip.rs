use crate::challenges::broadcast::send_gossip_to_peers;
use crate::challenges::cluster::global_cluster;
use crate::send;
use crate::{BodyBase, challenges::broadcast::prepare_gossip_batch, challenges::broadcast::spawn_gossip_thread};
use anyhow::{Ok, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Write;

use crate::{Message, challenges::broadcast::BroadcastData};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GossipBody {
    #[serde(flatten)]
    pub base: BodyBase,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gossip_data: Option<HashSet<u64>>,

    pub org_msg_id: u64,
    pub org_msg_src: String,
}

pub fn gossip(msg: Message<GossipBody>, output: &mut impl Write) -> Result<()> {
    let mut cluster = global_cluster().write().unwrap();
    let node = cluster.get_node_mut(&msg.dest).unwrap();
    if node.broadcast_data.is_none() {
        node.broadcast_data = Some(BroadcastData::new())
    }
    {
        let broadcast_data = node.broadcast_data.as_mut().unwrap();
        broadcast_data.extend(msg.body.gossip_data.unwrap());
    }
    if node.gossip_thread.is_none() {
        let handle = spawn_gossip_thread(node.id.clone());
        node.gossip_thread = Some(handle.thread().clone());
    }

    if msg.body.base.typ == "gossip_ok"
        || !node
            .broadcast_data
            .as_mut()
            .unwrap()
            .add_if_not_present(&msg.body.org_msg_src, msg.body.org_msg_id)
    {
        return Ok(());
    }

    let msg_id = node.get_next_id();
    let src = node.id.clone();
    let response: Message<GossipBody> = Message {
        src: node.id.clone(),
        dest: msg.src,
        body: GossipBody {
            base: BodyBase {
                typ: "gossip_ok".to_string(),
                in_reply_to: msg.body.base.msg_id,
                msg_id: Some(msg_id),
            },
            gossip_data: Some(node.broadcast_data.clone().unwrap().data),
            org_msg_id: msg.body.org_msg_id,
            org_msg_src: msg.body.org_msg_src.clone(),
        },
    };

    drop(cluster);
    let (src, data, peers) = prepare_gossip_batch(&src).unwrap();

    if !peers.is_empty() {
        send_gossip_to_peers(
            &src,
            &data,
            &peers,
            msg.body.org_msg_id,
            &msg.body.org_msg_src,
        );
    }

    return send(&response, output);
}
