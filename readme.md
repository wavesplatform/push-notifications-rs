# Installation and start

## Environment variables


### Common

| Env variable | Required | Default | Note                     |
| ------------ | -------- | ------- | ------------------------ |
| RUST_LOG     | NO       | INFO    |                          |
| PORT         | NO       | 8080    |                          |
| METRICS_PORT | NO       | 9090    |                          |
| PGHOST       | YES      |         |                          |
| PGPORT       | NO       | 5432    |                          |
| PGDATABASE   | YES      |         |                          |
| PGUSER       | YES      |         | `WRITE`, `DELETE` access |
| PGPASSWORD   | YES      |         |                          |


### Processor (common)

| Env variable           | Required | Default | Note                                       |
|------------------------| -------- | ------- |--------------------------------------------|
| LOKALISE_SDK_TOKEN     | YES      |         | API token from lokalise                    |
| LOKALISE_PROJECT_ID    | YES      |         | Project ID in lokalise                     |


### Processor (prices)

| Env variable           | Required | Default | Note                                       |
|------------------------| -------- | ------- |--------------------------------------------|
| ASSETS_SERVICE_URL     | YES      |         | Assets-service root url. No trailing slash |
| DATA_SERVICE_URL       | YES      |         | Data-service root url. No trailing slash   |
| BLOCKCHAIN_UPDATES_URL | YES      |         | Blockchain updates url                     |
| MATCHER_ADDRESS        | YES      |         | Matcher address (base58)                   |
| STARTING_HEIGHT        | NO       | None    | [Debug only] Blockchain height to start receiving notifications.<br/>If not set (or zero) uses current height from data  service. |


### Processor (orders)

| Env variable           | Required | Default | Note                                       |
|------------------------| -------- | ------- |--------------------------------------------|
| REDIS_HOSTNAME         | YES      |         | Redis instance hostname                    |
| REDIS_PORT             | NO       | 6379    | Redis port                                 |
| REDIS_USER             | NO       | default | Redis user, must have write permission     |
| REDIS_PASSWORD         | YES      |         | Redis password                             |
| REDIS_STREAM_NAME      | YES      |         | E.g. 'matcher.external.orders.execution'   |
| REDIS_GROUP_NAME       | YES      |         | E.g. 'push-notifications-service'          |
| REDIS_CONSUMER_NAME    | YES      |         | E.g. 'push-notifications-0'                |
| REDIS_BATCH_SIZE       | NO       | 100     | Number of stream items to query at once    |


### API

| Env variable                           | Required | Default | Note                                                        |
|----------------------------------------| -------- |---------|-------------------------------------------------------------|
| POOL_CONNECTION_TIMEOUT_SEC            | NO       | 5       | Database pool connection timeout, seconds                   |
| MAX_SUBSCRIPTIONS_PER_ADDRESS_PER_PAIR | NO       | 10      | Maximum number of price subscriptions per pair, per address |
| MAX_SUBSCRIPTIONS_PER_ADDRESS_TOTAL    | NO       | 50      | Maximum number of price subscriptions in total, per address |


### Sender

| Env variable                                     | Required | Default | Note                                               |
| ------------------------------------------------ | -------- | ------- | -------------------------------------------------- |
| FCM_API_KEY                                      | YES      |         | A token from FCM for sending messages to apps      |
| SEND_EMPTY_QUEUE_POLL_PERIOD_MILLIS              | NO       | 5000    | Period of polling for new messages                 |
| SEND_EXPONENTIAL_BACKOFF_INITIAL_INTERVAL_MILLIS | NO       | 5000    | Message send exponential strategy initial interval |
| SEND_EXPONENTIAL_BACKOFF_MULTIPLIER              | NO       | 3.0     | Exponential strategy multiplier                    |
| SEND_MAX_ATTEMPTS                                | NO       | 5       | No more retries after reaching max attempts limit  |
| SEND_CLICK_ACTION                                | NO       | "open"  | "click_action" field in sent Notification          |
| SEND_DRY_RUN                                     | NO       | 5       | No more retries after reaching max attempts limit  |
