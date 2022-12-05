use chrono::{DateTime, Utc};
use diesel::{prelude::*, Connection, PgConnection};
use lib::{config::Config, Error};
use std::time::Duration;

// todo proper logging

const EMPTY_QUEUE_POLL_PERIOD: Duration = Duration::from_secs(5);

#[tokio::main]
async fn main() -> Result<(), Error> {
    // todo loop
    let config = Config::load()?;
    let mut conn = PgConnection::establish(&config.postgres.database_url())?;

    // get message from queue
    let message_to_send = postgres::dequeue(&mut conn)?;

    match message_to_send {
        None => {
            println!("NO MESSAGES | Sleep {:?}", EMPTY_QUEUE_POLL_PERIOD);
            tokio::time::sleep(EMPTY_QUEUE_POLL_PERIOD).await;
        }
        Some(message) => {
            // todo check eligibility to send using exponential backoff
            // for now, no retries. 1 attempt per message
            if message.send_attempts_count > 0 {
                println!("SKIP | Message {}", message.uid);
                postgres::skip(&mut conn, message.uid, Utc::now())?;
            } else {
                // message is eligible for sending
                let fcm_msg = formatter::message(&config.fcm_api_key, &message);

                // todo ttl

                match Ok::<fcm::Message, fcm::FcmError>(fcm_msg).map(|_| ()) {
                    // match fcm::Client::new().send(fcm_msg).await.map(|_| ()) {
                    Ok(()) => {
                        println!("SUCCESS | Message {}", message.uid);
                        postgres::ack(&mut conn, message.uid)?;
                        println!("Message {} deleted from DB", message.uid);
                    }
                    Err(err) => {
                        eprintln!("ERROR | Message {} | {:?}", message.uid, err);
                        postgres::nack(
                            &mut conn,
                            message.uid,
                            message.send_attempts_count + 1,
                            format!("{:?}", err),
                            Utc::now(),
                        )?;
                        println!("Message {} nack", message.uid);
                    }
                };
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Queryable)]
pub struct MessageToSend {
    pub uid: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub send_error: Option<String>,
    pub send_attempts_count: i32,
    pub notification_title: String,
    pub notification_body: String,
    pub collapse_key: Option<String>,
    pub fcm_uid: String,
}

// todo db transactions
mod postgres {
    use crate::MessageToSend;
    use chrono::{DateTime, Utc};
    use diesel::{prelude::*, PgConnection};
    use lib::{
        schema::{devices, messages},
        Error,
    };

    /// Sets message updated_at.
    /// Use to send message to the end of the queue with no action
    pub fn skip(
        conn: &mut PgConnection,
        message_uid: i32,
        new_updated_at: DateTime<Utc>,
    ) -> Result<(), Error> {
        diesel::update(messages::table)
            .filter(messages::uid.eq(message_uid))
            .set(messages::updated_at.eq(new_updated_at))
            .execute(conn)?;
        Ok(())
    }

    pub fn nack(
        conn: &mut PgConnection,
        message_uid: i32,
        new_send_attempts_count: i32,
        new_send_error: String,
        new_updated_at: DateTime<Utc>,
    ) -> Result<(), Error> {
        diesel::update(messages::table)
            .filter(messages::uid.eq(message_uid))
            .set((
                messages::updated_at.eq(new_updated_at),
                messages::send_attempts_count.eq(new_send_attempts_count),
                messages::send_error.eq(new_send_error),
            ))
            .execute(conn)?;
        Ok(())
    }

    pub fn ack(conn: &mut PgConnection, message_uid: i32) -> Result<(), Error> {
        diesel::delete(messages::table.filter(messages::uid.eq(message_uid))).execute(conn)?;
        Ok(())
    }

    pub fn dequeue(conn: &mut PgConnection) -> Result<Option<MessageToSend>, Error> {
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
            .filter(messages::send_attempts_count.lt(1)) // todo retries
            .order(messages::updated_at)
            .first(conn)
            .optional()?)
    }
}

mod formatter {
    use crate::MessageToSend;
    use std::collections::HashMap;

    pub fn message<'a>(
        fcm_api_key: &'a str,
        message_to_send: &'a MessageToSend,
    ) -> fcm::Message<'a> {
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
}

// #[tokio::test]
// async fn get_msg() {
//     let config = Config::load().unwrap();
//     let mut conn = PgConnection::establish(&config.postgres.database_url()).unwrap();
//     let msg = postgres::dequeue(&mut conn).unwrap().unwrap();
//     assert_eq!(msg.uid, 1);
//     assert_eq!(msg.fcm_uid, "uid_0");
// }
