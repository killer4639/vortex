use crate::challenges::broadcast::BroadcastData;

#[derive(Debug)]
#[allow(dead_code)]
pub struct Node {
    pub id: String,
    pub peers: Vec<String>,
    pub next_msg_id: u64,
    pub broadcast_data: Option<BroadcastData>
}

impl Node {
    pub fn get_next_id(&mut self) -> u64 {
        let msg_id = self.next_msg_id;
        self.next_msg_id = self.next_msg_id + 1;
        return msg_id;
    }
}
