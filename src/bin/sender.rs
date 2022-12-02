use diesel::{prelude::*, Connection, PgConnection};
use fcm::Message;
use lib::{config::PostgresConfig, schema::messages, Error};
use std::{collections::HashMap, time::Duration};

type UID = i32;

const FCM_API_KEY_TEMP: &str = "key-0";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let dbconfig = PostgresConfig::load()?;
    let mut conn = PgConnection::establish(&dbconfig.database_url())?;

    // get oldest message
    let (notification_title, notification_body, collapse_key): (String, String, Option<String>) =
        messages::table
            .select((
                messages::notification_title,
                messages::notification_body,
                messages::collapse_key,
            ))
            .filter(messages::sending_error.is_null())
            .order(messages::updated_at.asc())
            .first(&mut conn)?;

    // let client = fcm::Client::new();

    // let mut map = HashMap::new();
    // map.insert("message", "Howdy!");

    let notification = {
        let mut builder = fcm::NotificationBuilder::new();
        builder.title(&notification_title);
        builder.body(&notification_body);
        builder.finalize()
    };

    // todo get from db
    let recipient_fcm_uid = String::from("vasya");

    let message = {
        let mut builder = fcm::MessageBuilder::new(FCM_API_KEY_TEMP, &recipient_fcm_uid);
        builder.notification(notification);
        builder.data(&HashMap::<String, String>::new())?; // message must have `data` field, or empty object as minimum

        // todo collapse key
        // if let Some(k) = collapse_key {
        // builder.collapse_key(&k);
        // }

        builder.finalize()

        // todo ttl
        // todo priority
    };

    match send_to_fcm(message).await {
        Ok(()) => println!("Send succesful; TODO delete"),
        Err(err) => eprintln!("Send error: {:?}. TODO set updated_at, sending_error", err),
    };

    Ok(())
}

async fn send_to_fcm<'a>(message: Message<'a>) -> anyhow::Result<()> {
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("Message: {:?}", message);
    Ok(())
}
