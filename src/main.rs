mod challenges;
use std::io::{self, Write};

use anyhow::{Context, Result};
use challenges::echo::EchoBody;
use challenges::generate::GenerateBody;
use challenges::init::InitBody;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::challenges::gcounter::{AddBody, GossipBody, ReadBody};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message<T> {
    pub src: String,
    pub dest: String,
    pub body: T,
}

#[derive(Debug, Clone)]
pub enum TypedMessage {
    Init(Message<InitBody>),
    Echo(Message<EchoBody>),
    Generate(Message<GenerateBody>),
    Read(Message<ReadBody>),
    Add(Message<AddBody>),
    Gossip(Message<GossipBody>),
    Unknown(Message<Value>),
}

impl<T> Message<T> {
    /// Creates a reply message with the given body, swapping src/dest.
    pub fn into_reply<U>(self, body: U) -> Message<U> {
        Message {
            src: self.dest,
            dest: self.src,
            body,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BodyBase {
    #[serde(rename = "type")]
    pub typ: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg_id: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<u64>,
}

pub fn parse_typed_message(msg: Message<Value>) -> Result<TypedMessage> {
    let typ = msg
        .body
        .get("type")
        .and_then(|value| value.as_str())
        .context("message body missing type")?;

    match typ {
        "init" => Ok(TypedMessage::Init(parse_message(msg)?)),
        "echo" => Ok(TypedMessage::Echo(parse_message(msg)?)),
        "generate" => Ok(TypedMessage::Generate(parse_message(msg)?)),
        "read" => Ok(TypedMessage::Read(parse_message(msg)?)),
        "add" => Ok(TypedMessage::Add(parse_message(msg)?)),
        "gossip" => Ok(TypedMessage::Gossip(parse_message(msg)?)),
        _ => Ok(TypedMessage::Unknown(msg)),
    }
}

pub fn parse_message<T: DeserializeOwned>(msg: Message<Value>) -> Result<Message<T>> {
    let body = serde_json::from_value(msg.body)?;
    Ok(Message {
        src: msg.src,
        dest: msg.dest,
        body,
    })
}

pub fn send<T: Serialize>(msg: &Message<T>, output: &mut impl Write) -> Result<()> {
    serde_json::to_writer(&mut *output, msg)?;
    output.write_all(b"\n")?;
    output.flush()?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let stdin = io::stdin().lock();
    let mut stdout = io::stdout();
    let messages = serde_json::Deserializer::from_reader(stdin).into_iter::<Message<Value>>();

    // Process remaining messages
    for msg in messages {
        let msg = msg?;
        let msg_typ = parse_typed_message(msg)?;

        match msg_typ {
            TypedMessage::Init(msg) => challenges::init::init(msg, &mut stdout)?,
            TypedMessage::Echo(msg) => challenges::echo::echo(msg, &mut stdout)?,
            TypedMessage::Generate(msg) => {
                challenges::generate::generate_unique_id(msg, &mut stdout)?
            }
            TypedMessage::Read(msg) => challenges::gcounter::read(msg, &mut stdout)?,
            TypedMessage::Add(msg) => challenges::gcounter::add(msg, &mut stdout)?,
            TypedMessage::Gossip(msg) => challenges::gcounter::gossip(msg, &mut stdout)?,
            TypedMessage::Unknown(_msg) => {}
        }
    }
    Ok(())
}
