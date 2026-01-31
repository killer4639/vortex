use crate::{BodyBase, Message, challenges::cluster::global_cluster};
use anyhow::{Context, Ok, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GenerateBody {
    #[serde(flatten)]
    body: BodyBase,

    id: Option<String>,
}

pub fn generate_unique_id(msg: Message<GenerateBody>) -> Result<Message<GenerateBody>> {
    let node_id = msg.dest.clone();
    let mut cluster = global_cluster().write().expect("cluster lock poisoned");
    let node = cluster
        .get_node_mut(&node_id)
        .context("node not found in cluster")?;

    let unique_id = Uuid::new_v4().to_string();
    let response: Message<GenerateBody> = Message {
        src: node.id.clone(),
        dest: msg.src.clone(),
        body: GenerateBody {
            id: Some(unique_id),
            body: BodyBase {
                typ: "generate_ok".to_string(),
                msg_id: Some(node.get_next_id()),
                in_reply_to: msg.body.body.msg_id,
            },
        },
    };
    Ok(response)
}
