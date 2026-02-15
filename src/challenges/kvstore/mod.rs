use std::io::Write;

use crate::{BodyBase, Message, challenges::cluster::global_cluster, send};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TxnBody {
    #[serde(flatten)]
    body: BodyBase,

    txn: Vec<(String, i64, Option<i64>)>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReplicateBody {
    #[serde(flatten)]
    body: BodyBase,

    txn: (i64, i64),
}

pub fn transaction(msg: Message<TxnBody>, output: &mut impl Write) -> Result<()> {
    let mut response_txns: Vec<(String, i64, Option<i64>)> = Vec::new();
    for txn in msg.body.txn {
        let ops = txn.0.clone();
        let key = txn.1.clone();
        let result = match ops.as_str() {
            "r" => read(&msg.dest, txn),
            "w" => write(&msg.dest, txn),
            _ => bail!("unknown kvstore op: {ops}"),
        };

        response_txns.push((ops, key, result.unwrap()));
    }

    let mut cluster = global_cluster().write().unwrap();
    let node = cluster.get_node_mut(&msg.dest).unwrap();
    let response = Message {
        src: msg.dest.clone(),
        dest: msg.src.clone(),
        body: TxnBody {
            body: BodyBase {
                typ: "txn_ok".to_string(),
                msg_id: Some(node.get_next_id()),
                in_reply_to: msg.body.body.msg_id,
            },
            txn: response_txns,
        },
    };

    send(&response, output)
}

pub fn read(node_id: &String, transaction: (String, i64, Option<i64>)) -> Result<Option<i64>> {
    let mut cluster = global_cluster().write().unwrap();
    let node = cluster.get_node_mut(node_id).unwrap();

    let data = node.commited.get(&transaction.1);
    let response = if data.is_none() {
        None
    } else {
        Some(data.unwrap().clone())
    };
    Ok(response)
}
pub fn write(node_id: &String, transaction: (String, i64, Option<i64>)) -> Result<Option<i64>> {
    let mut cluster = global_cluster().write().unwrap();
    let node = cluster.get_node_mut(node_id).unwrap();

    node.pending
        .insert(transaction.1.clone(), transaction.2.unwrap().clone());
    drop(cluster);
    make_pending_request(node_id);
    Ok(transaction.2)
}


pub fn make_pending_request(node_id: &String) -> () {}