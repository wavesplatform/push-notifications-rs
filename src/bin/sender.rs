use diesel::{prelude::*, Connection, PgConnection};
use lib::{
    config::Config,
    schema::{devices, messages, subscriptions},
    Error,
};
use std::collections::HashMap;

type UID = i32;
// todo proper logging

#[tokio::main]
async fn main() -> Result<(), Error> {
    let config = Config::load()?;
    let mut conn = PgConnection::establish(&config.postgres.database_url())?;

    // get oldest message
    let (message_uid, subscription_uid, notification_title, notification_body, _collapse_key): (
        UID,
        UID,
        String,
        String,
        Option<String>,
    ) = messages::table
        .select((
            messages::uid,
            messages::subscription_uid,
            messages::notification_title,
            messages::notification_body,
            messages::collapse_key,
        ))
        .filter(messages::sending_error.is_null())
        .order(messages::updated_at.asc())
        .first(&mut conn)?; // todo handle Error: Record not found

    // get recipient `fcm_uid` from message `subscription_uid`
    // todo multiple devices support
    let recipient_fcm_uid: String = devices::table
        .select(devices::fcm_uid)
        .inner_join(
            subscriptions::table
                .on(subscriptions::subscriber_address.eq(devices::subscriber_address)),
        )
        .filter(subscriptions::uid.eq(subscription_uid))
        .first(&mut conn)?;

    let notification = {
        let mut builder = fcm::NotificationBuilder::new();
        builder.title(&notification_title);
        builder.body(&notification_body);
        builder.finalize()
    };

    let message = {
        let mut builder = fcm::MessageBuilder::new(&config.fcm_api_key, &recipient_fcm_uid);
        builder.notification(notification);
        builder.data(&HashMap::<String, String>::new())?; // message must have `data` field, or empty object as minimum

        // todo collapse key
        // if let Some(k) = collapse_key {
        // builder.collapse_key(&k);
        // }

        // todo ttl
        // todo priority

        builder.finalize()
    };

    // todo post-send actions
    // match fcm::Client::new().send(message).await.map(|_| ()) {
    match Ok::<_, fcm::FcmError>(message).map(|_| ()) {
        Ok(()) => {
            println!("Send succesful message UID {}", message_uid);

            diesel::delete(messages::table.filter(messages::uid.eq(message_uid)))
                .execute(&mut conn)?;

            println!("Message {} deleted from DB", message_uid);
        }
        Err(err) => eprintln!("Send error: {:?}. TODO set updated_at, sending_error", err),
    };

    Ok(())
}
