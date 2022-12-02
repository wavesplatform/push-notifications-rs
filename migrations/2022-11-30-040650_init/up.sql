CREATE TABLE IF NOT EXISTS subscribers (
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    address varchar NOT NULL primary key
);

CREATE TABLE IF NOT EXISTS devices (
    uid serial not null primary key,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    fcm_uid varchar NOT NULL,
    subscriber_address varchar NOT NULL,
    language varchar NOT NULL,
    foreign key (subscriber_address) references subscribers(address)
);
create index on devices(subscriber_address);

CREATE TABLE IF NOT EXISTS subscriptions (
    uid serial not null unique,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    subscriber_address varchar NOT NULL,
    topic varchar NOT NULL,
    topic_type integer not null, -- todo enum instead of integer
    primary key (subscriber_address, topic),
    foreign key (subscriber_address) references subscribers(address)
);

-- Topic-specific tables. Fields are indexed and search-optimized.
CREATE TABLE IF NOT EXISTS topics_price_threshold (
    subscription_uid integer primary key,
    amount_asset_id varchar not null,
    price_asset_id varchar not null,
    price_threshold bigint not null,
    foreign key (subscription_uid) references subscriptions(uid)
);
create index on topics_price_threshold(amount_asset_id, price_asset_id, price_threshold);

CREATE TABLE IF NOT EXISTS messages (
    uid serial not null primary key,
    created_at timestamptz not null default now(),
    subscription_uid integer not null,
    notification_title varchar not null,
    notification_body varchar not null,
    data jsonb,
    collapse_key varchar,
    foreign key (subscription_uid) references subscriptions(uid)
);
create index on messages(subscription_uid);

CREATE TABLE IF NOT EXISTS failed_send_attempts (
    attempted_at timestamptz not null default now(),
    message_uid integer not null,
    device_uid integer not null,
    error_reason varchar not null,
    primary key (message_uid, attempted_at, device_uid),
    foreign key (message_uid) references messages(uid),
    foreign key (device_uid) references devices(uid)
);
create index on failed_send_attempts(device_uid);
