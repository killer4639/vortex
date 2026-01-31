mod challenges;
use std::fs::OpenOptions;
use std::io::{self, BufWriter, Write};

use anyhow::{Context, Result};
use challenges::echo::EchoBody;
use challenges::init::InitBody;
use challenges::generate::GenerateBody;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

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

pub fn message_to_value<T: Serialize>(msg: Message<T>) -> Result<Message<Value>> {
    let body = serde_json::to_value(msg.body)?;
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
    let mut stdout = BufWriter::new(io::stdout().lock());

    let messages = serde_json::Deserializer::from_reader(stdin).into_iter::<Message<Value>>();

    // Process remaining messages
    for msg in messages {
        let msg = msg?;
        let msg_typ = parse_typed_message(msg)?;

        let response = match msg_typ {
            TypedMessage::Init(msg) => message_to_value(challenges::init::init(msg)?),
            TypedMessage::Echo(msg) => message_to_value(challenges::echo::echo(msg)?),
            TypedMessage::Generate(msg) => message_to_value(challenges::generate::generate_unique_id(msg)?),
            TypedMessage::Unknown(msg) => Ok(msg),
        }?;

        send(&response, &mut stdout)?;
    }
    Ok(())
}
