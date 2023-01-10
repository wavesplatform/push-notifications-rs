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

### Processor

| Env variable           | Required | Default | Note                                       |
|------------------------| -------- | ------- |--------------------------------------------|
| LOKALISE_SDK_TOKEN     | YES      |         | API token from lokalise                    |
| ASSETS_SERVICE_URL     | YES      |         | Assets-service root url. No trailing slash |
| DATA_SERVICE_URL       | YES      |         | Data-service root url. No trailing slash   |
| BLOCKCHAIN_UPDATES_URL | YES      |         | Blockchain updates url                     |
| MATCHER_ADDRESS        | YES      |         | Matcher address (base58)                   |
| STARTING_HEIGHT        | NO       | None    | [Debug only] Blockchain height to start receiving notifications.<br/>If not set (or zero) uses current height from data  service. |

### API

Only [common](###Common)

### Sender

| Env variable                                     | Required | Default | Note                                               |
| ------------------------------------------------ | -------- | ------- | -------------------------------------------------- |
| FCM_API_KEY                                      | YES      |         | A token from FCM for sending messages to apps      |
| SEND_EMPTY_QUEUE_POLL_PERIOD_MILLIS              | NO       | 5000    | Period of polling for new messages                 |
| SEND_EXPONENTIAL_BACKOFF_INITIAL_INTERVAL_MILLIS | NO       | 5000    | Message send exponential strategy initial interval |
| SEND_EXPONENTIAL_BACKOFF_MULTIPLIER              | NO       | 3.0     | Exponential strategy multiplier                    |
| SEND_SEND_MAX_ATTEMPTS                           | NO       | 5       | No more retries after reaching max attempts limit  |
