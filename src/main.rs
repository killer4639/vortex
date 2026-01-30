//! Maelstrom Echo Node Implementation
//!
//! A distributed systems workbench node that handles echo protocol messages.

use std::io::{self, BufWriter, Write};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// ============================================================================
// Protocol Types
// ============================================================================

/// Represents a message in the Maelstrom protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message {
    src: String,
    dest: String,
    body: Body,
}

impl Message {
    /// Creates a reply message with the given body, swapping src/dest.
    fn into_reply(self, body: Body) -> Self {
        Self {
            src: self.dest,
            dest: self.src,
            body,
        }
    }
}

/// Message body containing the payload and metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Body {
    #[serde(rename = "type")]
    typ: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    msg_id: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    in_reply_to: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    echo: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    node_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    node_ids: Option<Vec<String>>,
}

// ============================================================================
// Node Implementation
// ============================================================================

/// An echo node that responds to echo requests.
#[derive(Debug)]
#[allow(dead_code)]
struct EchoNode {
    id: String,
    peers: Vec<String>,
    next_msg_id: u64,
}

impl EchoNode {
    /// Creates a new EchoNode from an init message.
    fn from_init(msg: &Message) -> Result<Self> {
        let id = msg
            .body
            .node_id
            .clone()
            .context("init message missing node_id")?;

        let peers = msg
            .body
            .node_ids
            .clone()
            .context("init message missing node_ids")?;

        Ok(Self {
            id,
            peers,
            next_msg_id: 0,
        })
    }

    /// Generates and returns the next message ID.
    fn next_msg_id(&mut self) -> u64 {
        let id = self.next_msg_id;
        self.next_msg_id += 1;
        id
    }

    /// Handles an incoming message and writes the response.
    fn handle(&mut self, msg: Message, output: &mut impl Write) -> Result<()> {
        let reply = msg.clone().into_reply(Body {
            typ: "echo_ok".into(),
            msg_id: Some(self.next_msg_id()),
            in_reply_to: msg.body.msg_id,
            echo: msg.body.echo,
            ..Default::default()
        });

        send(&reply, output)
    }
}

// ============================================================================
// I/O Helpers
// ============================================================================

/// Sends a message as JSON followed by a newline.
fn send(msg: &Message, output: &mut impl Write) -> Result<()> {
    serde_json::to_writer(&mut *output, msg)?;
    output.write_all(b"\n")?;
    output.flush()?;
    Ok(())
}

/// Replies to an init message with init_ok.
fn reply_init_ok(msg: Message, output: &mut impl Write) -> Result<()> {
    let reply = msg.clone().into_reply(Body {
        typ: "init_ok".into(),
        in_reply_to: msg.body.msg_id,
        ..Default::default()
    });

    send(&reply, output)
}

// ============================================================================
// Main
// ============================================================================

fn main() -> Result<()> {
    let stdin = io::stdin().lock();
    let mut stdout = BufWriter::new(io::stdout().lock());

    let mut messages = serde_json::Deserializer::from_reader(stdin).into_iter::<Message>();

    // First message must be init
    let init_msg = messages
        .next()
        .context("expected init message")?
        .context("failed to parse init message")?;

    anyhow::ensure!(init_msg.body.typ == "init", "first message must be init");

    let mut node = EchoNode::from_init(&init_msg)?;
    reply_init_ok(init_msg, &mut stdout)?;

    // Process remaining messages
    for msg in messages {
        let msg = msg.context("failed to parse message")?;
        node.handle(msg, &mut stdout)?;
    }

    Ok(())
}
