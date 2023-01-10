CREATE TABLE IF NOT EXISTS subscribers (
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    address varchar NOT NULL primary key
);

CREATE TABLE IF NOT EXISTS devices (
    uid serial not null unique,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    fcm_uid varchar NOT NULL,
    subscriber_address varchar NOT NULL,
    language varchar NOT NULL,
    utc_offset_seconds integer NOT NULL,
    CHECK (utc_offset_seconds >= -43200 AND utc_offset_seconds <= 50400),
    
    primary key (subscriber_address, fcm_uid),
    foreign key (subscriber_address) references subscribers(address)
);

CREATE TABLE IF NOT EXISTS subscriptions (
    uid serial not null unique,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    subscriber_address varchar NOT NULL,
    topic varchar NOT NULL,
    topic_type integer not null, -- todo enum instead of integer
    primary key (subscriber_address, topic),
    foreign key (subscriber_address) references subscribers(address) ON DELETE CASCADE
);

-- Topic-specific tables. Fields are indexed and search-optimized.
CREATE TABLE IF NOT EXISTS topics_price_threshold (
    subscription_uid integer primary key,
    amount_asset_id varchar not null,
    price_asset_id varchar not null,
    price_threshold double precision not null,

    UNIQUE (amount_asset_id, price_asset_id, price_threshold),
    foreign key (subscription_uid) references subscriptions(uid) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS messages (
    uid serial not null primary key,
    created_at timestamptz not null default now(),
    scheduled_for timestamptz not null default now(),
    send_attempts_count smallint not null default 0,
    send_error varchar,
    device_uid integer not null,
    notification_title varchar not null,
    notification_body varchar not null,
    data jsonb,
    collapse_key varchar,
    foreign key (device_uid) references devices(uid),
    constraint send_attempts_count_u8 check (send_attempts_count >= 0 and send_attempts_count < 256)
);
create index on messages(device_uid);

CREATE OR REPLACE FUNCTION unsub_cleanup()
    RETURNS TRIGGER AS $$
BEGIN
    IF NOT EXISTS (SELECT FROM subscriptions WHERE subscriber_address = OLD.subscriber_address)
    AND NOT EXISTS (SELECT FROM devices WHERE subscriber_address = OLD.subscriber_address) THEN
        DELETE from subscribers WHERE address = OLD.subscriber_address;
    END IF;
    RETURN OLD;
END;
$$
LANGUAGE plpgsql;

CREATE OR REPLACE TRIGGER trigger_unsub_cleanup AFTER DELETE ON subscriptions
    FOR EACH ROW
    EXECUTE PROCEDURE unsub_cleanup();