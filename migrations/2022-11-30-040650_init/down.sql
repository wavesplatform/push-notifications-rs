drop table messages, devices, topics_price_threshold, subscriptions, subscribers;
DROP TRIGGER IF EXISTS trigger_unsub_cleanup ON subscriptions;
DROP FUNCTION IF EXISTS unsub_cleanup;