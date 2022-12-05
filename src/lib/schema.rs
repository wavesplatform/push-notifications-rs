// @generated automatically by Diesel CLI.

diesel::table! {
    devices (uid) {
        uid -> Int4,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        fcm_uid -> Varchar,
        subscriber_address -> Varchar,
        language -> Varchar,
    }
}

diesel::table! {
    failed_send_attempts (message_uid, attempted_at, device_uid) {
        attempted_at -> Timestamptz,
        message_uid -> Int4,
        device_uid -> Int4,
        error_reason -> Varchar,
    }
}

diesel::table! {
    messages (uid) {
        uid -> Int4,
        created_at -> Timestamptz,
        subscription_uid -> Int4,
        notification_title -> Varchar,
        notification_body -> Varchar,
        data -> Nullable<Jsonb>,
        collapse_key -> Nullable<Varchar>,
    }
}

diesel::table! {
    subscribers (address) {
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        address -> Varchar,
    }
}

diesel::table! {
    subscriptions (subscriber_address, topic) {
        uid -> Int4,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        subscriber_address -> Varchar,
        topic -> Varchar,
        topic_type -> Int4,
    }
}

diesel::table! {
    topics_price_threshold (subscription_uid) {
        subscription_uid -> Int4,
        amount_asset_id -> Varchar,
        price_asset_id -> Varchar,
        price_threshold -> Int8,
    }
}

diesel::joinable!(devices -> subscribers (subscriber_address));
diesel::joinable!(failed_send_attempts -> devices (device_uid));
diesel::joinable!(failed_send_attempts -> messages (message_uid));
diesel::joinable!(subscriptions -> subscribers (subscriber_address));

diesel::joinable_inner!(messages (subscription_uid) -> subscriptions (uid));

diesel::allow_tables_to_appear_in_same_query!(
    devices,
    failed_send_attempts,
    messages,
    subscribers,
    subscriptions,
    topics_price_threshold,
);
