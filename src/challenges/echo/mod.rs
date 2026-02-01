use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{send, BodyBase, Message, challenges::cluster::global_cluster};
use std::io::Write;


#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EchoBody {
    #[serde(flatten)]
    pub base: BodyBase,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub echo: Option<String>,
}


pub fn echo(msg: Message<EchoBody>, output: &mut impl Write) -> Result<()> {
    let node_id = msg.dest.clone();
    let mut cluster = global_cluster()
        .write()
        .expect("cluster lock poisoned");
    let node = cluster
        .get_node_mut(&node_id)
        .context("node not found in cluster")?;

    let reply = Message {
        src: node.id.clone(),
        dest: msg.src,
        body: EchoBody {
            base: BodyBase {
                typ: "echo_ok".into(),
                msg_id: Some(node.get_next_id()),
                in_reply_to: msg.body.base.msg_id,
            },
            echo: msg.body.echo,
        },
    };

    send(&reply, output)
}
