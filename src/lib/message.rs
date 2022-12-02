use crate::{error::Error, WithTimestamp};
use std::collections::HashMap;

#[derive(Queryable)]
pub struct Message {
    notification_title: String,
    notification_body: String,
    data: HashMap<String, String>, // todo arbitrary JSON
    collapse_key: String,
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
