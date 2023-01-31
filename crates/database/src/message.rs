use diesel::ExpressionMethods;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use model::message::PreparedMessage;

use crate::{error::Error, schema::messages};

/// Message queue in the database
pub struct Queue {}

impl Queue {
    pub async fn enqueue(
        &self,
        message: PreparedMessage,
        conn: &mut AsyncPgConnection,
    ) -> Result<(), Error> {
        // This conversion can only fail due to a programming error
        // (see `to_value` docs), so using unwrap is safe and no need
        // to propagate error here
        let data = serde_json::to_value(message.data).expect("serialize json");

        let values = (
            messages::device_uid.eq(message.device.device_uid),
            messages::notification_title.eq(message.message.notification_title),
            messages::notification_body.eq(message.message.notification_body),
            messages::data.eq(data),
            messages::collapse_key.eq(message.collapse_key),
        );
        let num_rows = diesel::insert_into(messages::table)
            .values(values)
            .execute(conn)
            .await?;
        debug_assert_eq!(num_rows, 1); //TODO return error?

        Ok(())
    }
}
