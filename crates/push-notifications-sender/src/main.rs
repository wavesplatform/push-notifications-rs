//! Push notifications Sender service executable

extern crate wavesexchange_log as log;

mod backoff;
mod config;

use chrono::{DateTime, Utc};
use diesel::{prelude::*, Connection, PgConnection};
use std::fmt;
use tokio::task;
use wavesexchange_warp::MetricsWarpBuilder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configs
    let pg_config = database::config::Config::load()?;
    let config = config::Config::load()?;
    log::info!(
        "Starting push-notifications sender service with {:?}",
        config
    );

    log::info!("Connecting to postgres database: {:?}", pg_config);
    let mut conn = PgConnection::establish(&pg_config.database_url())?;

    let fcm = FcmRemoteGateway {
        client: fcm::Client::new(),
        api_key: config.fcm_api_key,
        click_action: config.click_action,
        dry_run: config.dry_run,
    };

    // Stats & liveness endpoints
    task::spawn(
        MetricsWarpBuilder::new()
            .with_metrics_port_from_env()
            .run_async(),
    );

    loop {
        let message_to_send = postgres::dequeue(&mut conn, config.send_max_attempts as i16)?;

        match message_to_send {
            None => {
                tokio::time::sleep(config.empty_queue_poll_period.to_std().unwrap()).await;
                // .unwrap() is safe, non-negativity is validated on config load (u32)
            }
            Some(message) => {
                // todo ttl
                match fcm.send(&message).await {
                    Ok(()) => {
                        log::info!("SENT message #{}", message.uid);
                        log::debug!("BODY: {:?}", message);
                        postgres::ack(&mut conn, message.uid)?;
                        log::debug!("DB DELETE message #{}", message.uid);
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

#[derive(Clone, Queryable)]
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

impl fmt::Debug for MessageToSend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Intentionally avoid printing fcm_uid for security reasons
        write!(
            f,
            "MessageToSend {{ uid: {}, created_at: {:?}, updated_at: {:?}, send_error: {:?}, send_attempts_count: {}, notification_title: {}, notification_body: {}, data: {:?}, collapse_key: {:?}, fcm_uid: *** }}",
            self.uid,
            self.created_at,
            self.updated_at,
            self.send_error,
            self.send_attempts_count,
            self.notification_title,
            self.notification_body,
            self.data,
            self.collapse_key,
        )
    }
}

struct FcmRemoteGateway {
    client: fcm::Client,
    api_key: String,
    click_action: String,
    dry_run: bool,
}

impl FcmRemoteGateway {
    pub async fn send(&self, message: &MessageToSend) -> anyhow::Result<()> {
        if !self.dry_run {
            let fcm_msg = self.fcm_message(&message);
            let fcm_response = self.client.send(fcm_msg).await?; // todo handle errors from FcmResponse body
            log::debug!("Message #{} {:?}", message.uid, fcm_response);
        }
        Ok(())
    }

    fn fcm_message<'a: 'b, 'b>(&'a self, message: &'b MessageToSend) -> fcm::Message<'b> {
        let notification = {
            let mut builder = fcm::NotificationBuilder::new();
            builder.title(&message.notification_title);
            builder.body(&message.notification_body);
            builder.click_action(&self.click_action);
            builder.finalize()
        };

        let mut builder = fcm::MessageBuilder::new(&self.api_key, &message.fcm_uid);
        builder.notification(notification);

        // message must have `data` field from DB or at least an empty object
        builder
            .data(message.data.as_ref().unwrap_or(&serde_json::json!("{}")))
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
    use database::schema::{devices, messages};
    use diesel::{prelude::*, PgConnection};

    // todo separate business logic from DB I/O
    pub fn nack(
        conn: &mut PgConnection,
        message_uid: i32,
        new_send_attempts_count: i16,
        new_send_error: String,
        new_scheduled_for: DateTime<Utc>,
    ) -> anyhow::Result<()> {
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

    pub fn ack(conn: &mut PgConnection, message_uid: i32) -> anyhow::Result<()> {
        diesel::delete(messages::table.filter(messages::uid.eq(message_uid))).execute(conn)?;
        Ok(())
    }

    pub fn dequeue(
        conn: &mut PgConnection,
        max_send_attempts: i16,
    ) -> anyhow::Result<Option<MessageToSend>> {
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
