use chrono::{DateTime, Utc};
use diesel::{prelude::*, Connection, PgConnection};
use lib::{
    config::Config,
    schema::{devices, messages},
    Error,
};
use std::{collections::HashMap, time::Duration};

// todo proper logging

const EMPTY_QUEUE_POLL_PERIOD_SECS: u64 = 5;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let config = Config::load()?;
    let mut conn = PgConnection::establish(&config.postgres.database_url())?;

    // get message from queue
    let message_to_send = dequeue(&mut conn)?;

    match message_to_send {
        None => tokio::time::sleep(Duration::from_secs(EMPTY_QUEUE_POLL_PERIOD_SECS)).await,
        Some(msg) => {
            // todo ttl
            let message = fcm_message(&config.fcm_api_key, &msg);

            // todo post-send actions
            // match fcm::Client::new().send(message).await.map(|_| ()) {
            match Ok::<_, fcm::FcmError>(message).map(|_| ()) {
                Ok(()) => {
                    println!("Send succesful message UID {}", msg.uid);

                    diesel::delete(messages::table.filter(messages::uid.eq(msg.uid)))
                        .execute(&mut conn)?;

                    println!("Message {} deleted from DB", msg.uid);
                }
                Err(err) => eprintln!("Send error: {:?}. TODO set updated_at, sending_error", err),
            };
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Queryable)]
struct MessageToSend {
    pub uid: i32,
    pub _created_at: DateTime<Utc>,
    pub _updated_at: DateTime<Utc>,
    pub _send_error: Option<String>,
    pub _send_attempts_count: i32,
    pub notification_title: String,
    pub notification_body: String,
    pub _collapse_key: Option<String>,
    pub fcm_uid: String,
}

fn dequeue(conn: &mut PgConnection) -> Result<Option<MessageToSend>, Error> {
    Ok(messages::table
        .inner_join(devices::table.on(messages::device_uid.eq(devices::uid)))
        .select((
            messages::uid,
            messages::created_at,
            messages::updated_at,
            messages::send_error,
            messages::send_attempts_count,
            messages::notification_title,
            messages::notification_body,
            messages::collapse_key,
            devices::fcm_uid,
        ))
        .order(messages::updated_at)
        .first(conn)
        .optional()?)
}

fn fcm_message<'a>(fcm_api_key: &'a str, message_to_send: &'a MessageToSend) -> fcm::Message<'a> {
    let notification = {
        let mut builder = fcm::NotificationBuilder::new();
        builder.title(&message_to_send.notification_title);
        builder.body(&message_to_send.notification_body);
        builder.finalize()
    };

    let mut builder = fcm::MessageBuilder::new(fcm_api_key.as_ref(), &message_to_send.fcm_uid);
    builder.notification(notification);
    builder.data(&HashMap::<String, String>::new()).unwrap(); // message must have `data` field, or empty object as a minimum

    // todo collapse key
    // if let Some(k) = collapse_key {
    // builder.collapse_key(&k);
    // }

    // todo ttl
    // todo priority

    builder.finalize()
}

// #[tokio::test]
// async fn get_msg() {
//     let config = Config::load().unwrap();
//     let mut conn = PgConnection::establish(&config.postgres.database_url()).unwrap();
//     let msg = dequeue(&mut conn).unwrap().unwrap();
//     assert_eq!(msg.uid, 1);
//     assert_eq!(msg.fcm_uid, "uid_0");
// }
