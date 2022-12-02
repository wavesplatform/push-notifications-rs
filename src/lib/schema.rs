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
    messages (uid) {
        uid -> Int4,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        subscription_uid -> Int4,
        notification_title -> Varchar,
        notification_body -> Varchar,
        data -> Nullable<Jsonb>,
        collapse_key -> Nullable<Varchar>,
        sending_error -> Nullable<Varchar>,
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
diesel::joinable!(subscriptions -> subscribers (subscriber_address));

diesel::allow_tables_to_appear_in_same_query!(
    devices,
    messages,
    subscribers,
    subscriptions,
    topics_price_threshold,
);
