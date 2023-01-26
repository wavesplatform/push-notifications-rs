use crate::db::PgAsyncPool;
use database::{device, subscription};
use error::Error;
use model::Address;
use std::sync::Arc;
use warp::{Filter, Rejection};
use wavesexchange_warp::{
    error::{error_handler_with_serde_qs, handler, internal, validation},
    log::access,
    MetricsWarpBuilder,
};

const ERROR_CODES_PREFIX: u16 = 95;

type Pool = Arc<PgAsyncPool>;

pub async fn start(
    port: u16,
    metrics_port: u16,
    devices: device::Repo,
    subscriptions: subscription::Repo,
    pool: PgAsyncPool,
) {
    let error_handler = handler(ERROR_CODES_PREFIX, |err| match err {
        Error::ValidationError(field, error_details) => {
            let mut error_details = error_details.to_owned();
            if let Some(details) = error_details.as_mut() {
                details.insert("parameter".to_owned(), field.to_owned());
            }
            validation::invalid_parameter(ERROR_CODES_PREFIX, error_details)
        }
        Error::DbQueryError(e) => {
            log::error!(e);
            validation::invalid_parameter(ERROR_CODES_PREFIX, None)
        }
        _ => internal(ERROR_CODES_PREFIX),
    });

    let with_devices = warp::any().map(move || devices.clone());
    let with_subscriptions = warp::any().map(move || subscriptions.clone());

    let with_pool = {
        let pool = Arc::new(pool);
        warp::any().map(move || pool.clone())
    };

    let fcm_uid = warp::header::<String>("X-Fcm-Uid");
    let user_addr = warp::header::<String>("X-User-Address").and_then(|addr: String| async move {
        Address::from_string(&addr)
            .map_err(|e| Error::AddressParseError(e.to_string()))
            .map_err(Rejection::from)
    });

    let device_unregister = warp::delete()
        .and(warp::path!("device"))
        .and(fcm_uid)
        .and(user_addr)
        .and(with_devices.clone())
        .and(with_pool.clone())
        .and_then(controllers::unregister_device);

    let device_update = warp::patch()
        .and(warp::path!("device"))
        .and(fcm_uid)
        .and(user_addr)
        .and(with_devices.clone())
        .and(with_pool.clone())
        .and(warp::body::json::<dto::UpdateDevice>())
        .and_then(controllers::update_device);

    let device_register = warp::put()
        .and(warp::path!("device"))
        .and(fcm_uid)
        .and(user_addr)
        .and(with_devices.clone())
        .and(with_pool.clone())
        .and(warp::body::json::<dto::NewDevice>())
        .and_then(controllers::register_device);

    let topic_unsubscribe = warp::delete()
        .and(warp::path!("topics"))
        .and(user_addr)
        .and(with_subscriptions.clone())
        .and(with_pool.clone())
        .and(warp::body::json::<Option<dto::Topics>>())
        .and_then(controllers::unsubscribe_from_topics);

    let topic_subscribe = warp::post()
        .and(warp::path!("topics"))
        .and(user_addr)
        .and(with_subscriptions.clone())
        .and(with_pool.clone())
        .and(warp::body::json::<dto::Topics>())
        .and_then(controllers::subscribe_to_topics);

    let topics_get = warp::get()
        .and(warp::path!("topics"))
        .and(user_addr)
        .and(with_subscriptions.clone())
        .and(with_pool.clone())
        .and_then(controllers::get_topics);

    let log = warp::log::custom(access);

    log::info!("Starting push-notifications API server at 0.0.0.0:{}", port);

    let routes = device_unregister
        .or(device_update)
        .or(device_register)
        .or(topic_subscribe)
        .or(topic_unsubscribe)
        .or(topics_get)
        .recover(move |rej| {
            log::error!("{:?}", rej);
            error_handler_with_serde_qs(ERROR_CODES_PREFIX, error_handler.clone())(rej)
        })
        .with(log);

    MetricsWarpBuilder::new()
        .with_main_routes(routes)
        .with_main_routes_port(port)
        .with_metrics_port(metrics_port)
        .run_async()
        .await;
}

mod controllers {
    use super::{dto, Pool};
    use database::{
        device::{self, FcmUid},
        subscription::{self, SubscriptionRequest, Topic},
    };
    use diesel_async::AsyncConnection;
    use error::Error;
    use model::Address;
    use warp::{http::StatusCode, reply::Json, Rejection};

    use diesel_async::scoped_futures::ScopedFutureExt as _;

    pub async fn unregister_device(
        fcm_uid: FcmUid,
        address: Address,
        devices: device::Repo,
        pool: Pool,
    ) -> Result<StatusCode, Rejection> {
        pool.get()
            .await
            .map_err(Error::from)?
            .transaction(|conn| {
                async move {
                    // All work only within db transaction
                    devices.unregister(&address, &fcm_uid, conn).await
                }
                .scope_boxed()
            })
            .await?;

        Ok(StatusCode::NO_CONTENT)
    }

    pub async fn register_device(
        fcm_uid: FcmUid,
        address: Address,
        devices: device::Repo,
        pool: Pool,
        device_info: dto::NewDevice,
    ) -> Result<StatusCode, Rejection> {
        pool.get()
            .await
            .map_err(Error::from)?
            .transaction(|conn| {
                async move {
                    // All work only within db transaction
                    if devices.exists(&address, &fcm_uid, conn).await? {
                        return Ok::<StatusCode, Error>(StatusCode::NO_CONTENT);
                    }

                    devices
                        .register(
                            &address,
                            &fcm_uid,
                            &device_info.lang.language,
                            device_info.tz.utc_offset_seconds,
                            conn,
                        )
                        .await?;

                    Ok(StatusCode::CREATED)
                }
                .scope_boxed()
            })
            .await
            .map_err(Into::into)
    }

    pub async fn update_device(
        fcm_uid: FcmUid,
        address: Address,
        devices: device::Repo,
        pool: Pool,
        device_info: dto::UpdateDevice,
    ) -> Result<StatusCode, Rejection> {
        let has_fcm = device_info.fcm.is_some();

        pool.get()
            .await
            .map_err(Error::from)?
            .transaction(|conn| {
                async move {
                    // All work only within db transaction
                    devices
                        .update(
                            &address,
                            &fcm_uid,
                            device_info.lang.map(|l| l.language),
                            device_info.tz.map(|tz| tz.utc_offset_seconds),
                            device_info.fcm.map(|fcm| fcm.fcm_uid),
                            conn,
                        )
                        .await
                }
                .scope_boxed()
            })
            .await?;

        let response = if has_fcm {
            StatusCode::OK
        } else {
            StatusCode::NO_CONTENT
        };
        Ok(response)
    }

    pub async fn unsubscribe_from_topics(
        address: Address,
        subscriptions: subscription::Repo,
        pool: Pool,
        topics: Option<dto::Topics>,
    ) -> Result<StatusCode, Rejection> {
        pool.get()
            .await
            .map_err(Error::from)?
            .transaction(|conn| {
                async move {
                    // All work only within db transaction
                    subscriptions
                        .unsubscribe(&address, topics.map(|t| t.topics), conn)
                        .await
                }
                .scope_boxed()
            })
            .await?;

        Ok(StatusCode::NO_CONTENT)
    }

    pub async fn subscribe_to_topics(
        address: Address,
        subscriptions: subscription::Repo,
        pool: Pool,
        topics: dto::Topics,
    ) -> Result<StatusCode, Rejection> {
        let subs = topics
            .topics
            .into_iter()
            .map(|topic_url| {
                let (topic, mode) = Topic::from_url_string(&topic_url).map_err(|e| Error::BadTopic(e.to_string()))?;
                Ok(SubscriptionRequest {
                    // as_url_string removes junk from topic_url (like anchors or extra query params)
                    topic_url: topic.as_url_string(mode),
                    topic,
                    mode,
                })
            })
            .collect::<Result<Vec<SubscriptionRequest>, Error>>()?;

        pool.get()
            .await
            .map_err(Error::from)?
            .transaction(|conn| {
                async move {
                    // All work only within db transaction
                    subscriptions.subscribe(&address, subs, conn).await
                }
                .scope_boxed()
            })
            .await?;

        Ok(StatusCode::NO_CONTENT)
    }

    pub async fn get_topics(
        address: Address,
        subscriptions: subscription::Repo,
        pool: Pool,
    ) -> Result<Json, Rejection> {
        let topics = pool
            .get()
            .await
            .map_err(Error::from)?
            .transaction(|conn| {
                async move {
                    // All work only within db transaction
                    subscriptions.get_topics_by_address(&address, conn).await
                }
                .scope_boxed()
            })
            .await?;

        Ok(warp::reply::json(&dto::Topics { topics }))
    }
}

mod dto {
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize)]
    pub struct UpdateDevice {
        #[serde(flatten)]
        pub lang: Option<Lang>,
        #[serde(flatten)]
        pub tz: Option<Timezone>,
        #[serde(flatten)]
        pub fcm: Option<FcmUid>,
    }

    #[derive(Deserialize)]
    pub struct NewDevice {
        #[serde(flatten)]
        pub lang: Lang,
        #[serde(flatten)]
        pub tz: Timezone,
    }

    #[derive(Deserialize)]
    pub struct Lang {
        pub language: String,
    }

    #[derive(Deserialize)]
    pub struct Timezone {
        pub utc_offset_seconds: i32,
    }

    #[derive(Deserialize)]
    pub struct FcmUid {
        pub fcm_uid: String,
    }

    #[derive(Serialize, Deserialize)]
    pub struct Topics {
        pub topics: Vec<String>,
    }
}
