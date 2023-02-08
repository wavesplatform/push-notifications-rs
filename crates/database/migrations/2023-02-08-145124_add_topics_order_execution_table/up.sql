CREATE TABLE IF NOT EXISTS topics_order_execution (
    subscription_uid integer primary key,
    foreign key (subscription_uid) references subscriptions (uid) ON DELETE CASCADE
);

INSERT INTO topics_order_execution (subscription_uid)
    SELECT uid
    FROM subscriptions
    WHERE topic LIKE 'push://orders%';
