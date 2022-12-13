# Installation and start

## Environment variables

### Common

| Env variable | Required | Default | Note                     |
| ------------ | -------- | ------- | ------------------------ |
| RUST_LOG     | NO       | INFO    |                          |
| PGHOST       | YES      |         |                          |
| PGPORT       | NO       | 5432    |                          |
| PGDATABASE   | YES      |         |                          |
| PGUSER       | YES      |         | `WRITE`, `DELETE` access |
| PGPASSWORD   | YES      |         |                          |

### Processor

| Env variable       | Required | Default | Note                                       |
| ------------------ | -------- | ------- | ------------------------------------------ |
| LOKALISE_SDK_TOKEN | YES      |         | API token from lokalise                    |
| ASSETS_URL         | YES      |         | Assets-service root url. No trailing slash |

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
