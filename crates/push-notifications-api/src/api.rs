use crate::{db::PgAsyncPool, error::Error};
use database::{device, subscription};
use model::waves::Address;
use std::sync::Arc;
use warp::{http, Filter, Rejection};
use wavesexchange_warp::{
    error::{error_handler_with_serde_qs, handler, internal, validation, Response},
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
    subscribe_config: subscription::SubscribeConfig,
    pool: PgAsyncPool,
) {
    let error_handler = handler(ERROR_CODES_PREFIX, |err| match err {
        Error::DbQueryError(e) => {
            log::error!(e);
            validation::invalid_parameter(ERROR_CODES_PREFIX, None)
        }
        Error::DatabaseError(e @ database::error::Error::LimitExceeded(_, _)) => {
            log::debug!("{}", e);
            Response::singleton(
                http::StatusCode::BAD_REQUEST,
                "Too many subscriptions",
                ERROR_CODES_PREFIX as u32 * 10000 + 901,
                None,
            )
        }
        _ => internal(ERROR_CODES_PREFIX),
    });

    let with_devices = warp::any().map(move || devices.clone());
    let with_subscriptions = warp::any().map(move || subscriptions.clone());
    let with_subscribe_config = warp::any().map(move || subscribe_config.clone());

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
        .and(with_subscribe_config.clone())
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
    use crate::{
        error::Error,
        topic::{build_subscription_url, parse_subscription_url},
    };
    use database::{
        device,
        subscription::{self, SubscriptionRequest},
    };
    use diesel_async::AsyncConnection;
    use model::{device::FcmUid, waves::Address};
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
            .await
            .map_err(|e| Error::from(e))?;

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
            .await
            .map_err(|e| Error::from(e))?;

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
        let topics = topics
            .map(|t| {
                t.topics
                    .into_iter()
                    .map(|topic_url| {
                        // Subscription mode (`?oneshot`) is allowed but ignored here,
                        // so that the subscriber doesn't necessarily need to know it
                        // to be able to unsubscribe.
                        let (topic, _) = parse_subscription_url(&topic_url)?;
                        Ok(topic)
                    })
                    .collect::<Result<Vec<_>, Error>>()
            })
            .transpose()?;

        pool.get()
            .await
            .map_err(Error::from)?
            .transaction(|conn| {
                async move {
                    // All work only within db transaction
                    if let Some(topics) = topics {
                        subscriptions.unsubscribe(&address, topics, conn).await
                    } else {
                        subscriptions.unsubscribe_all(&address, conn).await
                    }
                }
                .scope_boxed()
            })
            .await
            .map_err(|e| Error::from(e))?;

        Ok(StatusCode::NO_CONTENT)
    }

    pub async fn subscribe_to_topics(
        address: Address,
        subscriptions: subscription::Repo,
        subscribe_config: subscription::SubscribeConfig,
        pool: Pool,
        topics: dto::Topics,
    ) -> Result<StatusCode, Rejection> {
        let subs = topics
            .topics
            .into_iter()
            .map(|topic_url| {
                let (topic, mode) = parse_subscription_url(&topic_url)?;
                Ok(SubscriptionRequest {
                    topic_url, // Can be safely removed
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
                    subscriptions
                        .subscribe(&address, subs, &subscribe_config, conn)
                        .await
                }
                .scope_boxed()
            })
            .await
            .map_err(|e| Error::from(e))?;

        Ok(StatusCode::NO_CONTENT)
    }

    pub async fn get_topics(
        address: Address,
        subscriptions: subscription::Repo,
        pool: Pool,
    ) -> Result<Json, Rejection> {
        let subscriptions = pool
            .get()
            .await
            .map_err(Error::from)?
            .transaction(|conn| {
                async move {
                    // All work only within db transaction
                    subscriptions.subscriptions_by_address(&address, conn).await
                }
                .scope_boxed()
            })
            .await
            .map_err(|e| Error::from(e))?;

        let topics = subscriptions
            .into_iter()
            .map(|(topic, mode)| build_subscription_url(topic, mode))
            .collect();

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
