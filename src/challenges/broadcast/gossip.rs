use crate::BodyBase;
use crate::challenges::cluster::global_cluster;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::{Message, challenges::broadcast::BroadcastData};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GossipBody {
    #[serde(flatten)]
    pub base: BodyBase,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gossip_data: Option<HashSet<u64>>,
}

pub fn gossip(msg: Message<GossipBody>) -> Message<GossipBody> {
    let mut cluster = global_cluster().write().unwrap();
    let node = cluster.get_node_mut(&msg.dest).unwrap();
    if node.broadcast_data.is_none() {
        node.broadcast_data = Some(BroadcastData {
            data: HashSet::new(),
        })
    }
    {
        let broadcast_data = node.broadcast_data.as_mut().unwrap();
        broadcast_data.merge_sets(msg.body.gossip_data.unwrap());
    }

    let msg_id = node.get_next_id();
    let src = node.id.clone();

    let response: Message<GossipBody> = Message {
        src,
        dest: msg.src,
        body: GossipBody {
            base: BodyBase {
                typ: "gossip_ok".to_string(),
                in_reply_to: msg.body.base.msg_id,
                msg_id: Some(msg_id),
            },
            gossip_data: None,
        },
    };
    response
}
