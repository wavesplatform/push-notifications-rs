CREATE TABLE IF NOT EXISTS subscribers (
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    address varchar NOT NULL primary key
);

CREATE TABLE IF NOT EXISTS devices (
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    fcm_uid varchar NOT NULL primary key,
    subscriber_address varchar NOT NULL references subscribers(address),
    language varchar NOT NULL
);

CREATE TYPE subscription_mode AS ENUM ('Once', 'Repeat');

CREATE TABLE IF NOT EXISTS subscriptions (
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    uid serial not null unique,
    mode subscription_mode not null,
    subscriber_address varchar NOT NULL references subscribers(address),
    topic varchar NOT NULL,
    primary key (subscriber_address, topic)
);

-- Topic-specific tables. Fields are indexed and search-optimized.
CREATE TABLE IF NOT EXISTS topics_price_threshold (
    subscription_uid integer references subscriptions(uid) primary key,
    amount_asset_id varchar not null,
    price_asset_id varchar not null,
    price_threshold bigint not null
);
create index on topics_price_threshold(amount_asset_id, price_asset_id, price_threshold);


CREATE TABLE IF NOT EXISTS messages (
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    subscription_uid integer references subscriptions(uid),
    localized_text varchar not null,
    -- TODO more fields? See what FCM requires
    primary key (subscription_uid, created_at)
);


drop table subscriptions cascade;
drop table subscribers cascade;
