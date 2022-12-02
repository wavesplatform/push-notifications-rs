use crate::{error::Error, WithTimestamp};
use std::collections::HashMap;

#[derive(Debug, Clone, Queryable)]
pub struct Message {
    pub notification_title: String,
    pub notification_body: String,
    pub data: HashMap<String, String>, // todo arbitrary JSON
    pub collapse_key: Option<String>,
}

pub struct Queue {}

impl Queue {
    pub async fn enqueue(
        subscription_uid: i32,
        message: WithTimestamp<Message>,
    ) -> Result<(), Error> {
        todo!("impl")
    }
}
