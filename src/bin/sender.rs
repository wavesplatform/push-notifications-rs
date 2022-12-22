//! Push notifications Sender service executable

extern crate wavesexchange_log as log;

use chrono::{DateTime, Utc};
use diesel::{prelude::*, Connection, PgConnection};
use lib::{
    backoff,
    config::{self, sender},
    Error,
};

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Configs
    let pg_config = config::postgres::Config::load()?;
    let config = sender::Config::load()?;
    log::info!("Starting push-notifications sender service with {:?}", config);

    log::info!("Connecting to postgres database: {:?}", pg_config);
    let mut conn = PgConnection::establish(&pg_config.database_url())?;

    loop {
        let message_to_send = postgres::dequeue(&mut conn, config.send_max_attempts as i16)?;

        match message_to_send {
            None => {
                log::debug!(
                    "No messages, sleep for {:?}s",
                    config.empty_queue_poll_period.num_seconds()
                );
                tokio::time::sleep(config.empty_queue_poll_period.to_std().unwrap()).await;
                // .unwrap() is safe, non-negativity is validated on config load (u32)
            }
            Some(message) => {
                let fcm_msg = message.to_fcm(&config.fcm_api_key);
                // todo ttl

                log::debug!("Sending {:?}", fcm_msg);

                match Ok::<fcm::Message, fcm::FcmError>(fcm_msg).map(|_| ()) {
                    // match fcm::Client::new().send(fcm_msg).await.map(|_| ()) {
                    Ok(()) => {
                        log::info!("SENT message {}", message.uid);
                        postgres::ack(&mut conn, message.uid)?;
                        log::debug!("Message {} deleted from DB", message.uid);
                    }
                    Err(err) => {
                        log::error!("Failed to send message {} | {:?}", message.uid, err);

                        let backoff_interval = backoff::exponential(
                            &config.exponential_backoff_initial_interval,
                            config.exponential_backoff_multiplier,
                            message.send_attempts_count,
                        );

                        let scheduled_for = Utc::now() + backoff_interval;

                        postgres::nack(
                            &mut conn,
                            message.uid,
                            message.send_attempts_count as i16 + 1,
                            format!("{:?}", err),
                            scheduled_for,
                        )?;

                        log::debug!(
                            "Message {} rescheduled for {:?} folowing backoff of {}s",
                            message.uid,
                            scheduled_for,
                            backoff_interval.num_seconds(),
                        );
                    }
                };
            }
        }
    }
}

#[derive(Debug, Clone, Queryable)]
pub struct MessageToSend {
    pub uid: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub send_error: Option<String>,
    #[diesel(deserialize_as = i16)]
    pub send_attempts_count: u8,
    pub notification_title: String,
    pub notification_body: String,
    pub data: Option<serde_json::Value>,
    pub collapse_key: Option<String>,
    pub fcm_uid: String,
}

impl MessageToSend {
    pub fn to_fcm<'a>(&'a self, fcm_api_key: &'a str) -> fcm::Message<'a> {
        let notification = {
            let mut builder = fcm::NotificationBuilder::new();
            builder.title(&self.notification_title);
            builder.body(&self.notification_body);
            builder.finalize()
        };

        let mut builder = fcm::MessageBuilder::new(fcm_api_key.as_ref(), &self.fcm_uid);
        builder.notification(notification);

        // message must have `data` field from DB or at least an empty object
        builder
            .data(self.data.as_ref().unwrap_or(&serde_json::json!("{}")))
            .unwrap(); // serde_json::Value guarantees success

        // todo collapse key
        // if let Some(k) = collapse_key {
        // builder.collapse_key(&k);
        // }

        // todo ttl
        // todo priority

        builder.finalize()
    }
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

    // todo separate business logic from DB I/O
    pub fn nack(
        conn: &mut PgConnection,
        message_uid: i32,
        new_send_attempts_count: i16,
        new_send_error: String,
        new_scheduled_for: DateTime<Utc>,
    ) -> Result<(), Error> {
        diesel::update(messages::table)
            .filter(messages::uid.eq(message_uid))
            .set((
                messages::scheduled_for.eq(new_scheduled_for),
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

    pub fn dequeue(
        conn: &mut PgConnection,
        max_send_attempts: i16,
    ) -> Result<Option<MessageToSend>, Error> {
        Ok(messages::table
            .inner_join(devices::table.on(messages::device_uid.eq(devices::uid)))
            .select((
                messages::uid,
                messages::created_at,
                messages::scheduled_for,
                messages::send_error,
                messages::send_attempts_count,
                messages::notification_title,
                messages::notification_body,
                messages::data,
                messages::collapse_key,
                devices::fcm_uid,
            ))
            .filter(messages::send_attempts_count.lt(max_send_attempts))
            .filter(messages::scheduled_for.lt(Utc::now()))
            .order(messages::scheduled_for)
            .first(conn)
            .optional()?)
    }
}

// todo remove or move to integration tests
// #[tokio::test]
// async fn get_msg() {
//     let config = Config::load().unwrap();
//     let mut conn = PgConnection::establish(&config.postgres.database_url()).unwrap();
//     let msg = postgres::dequeue(&mut conn).unwrap().unwrap();
//     assert_eq!(msg.uid, 1);
//     assert_eq!(msg.fcm_uid, "uid_0");
// }
