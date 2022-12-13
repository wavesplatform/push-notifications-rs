use crate::{config::Config, db::PgAsyncPool, device, model::Address, subscription, Error};
use diesel_async::{AsyncConnection, AsyncPgConnection};
use serde_json::{from_slice, Value};
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;
use warp::{reject, Filter, Rejection};
use wavesexchange_log::{error, info};
use wavesexchange_warp::error::{error_handler_with_serde_qs, handler, internal, validation};
use wavesexchange_warp::log::access;
use wavesexchange_warp::MetricsWarpBuilder;

const ERROR_CODES_PREFIX: u16 = 95;

type Pool = Arc<PgAsyncPool>;

pub async fn start(port: u16, metrics_port: u16, repos: Repos, pool: PgAsyncPool) {
    let error_handler = handler(ERROR_CODES_PREFIX, |err| match err {
        Error::ValidationError(field, error_details) => {
            let mut error_details = error_details.to_owned();
            if let Some(details) = error_details.as_mut() {
                details.insert("parameter".to_owned(), field.to_owned());
            }
            validation::invalid_parameter(ERROR_CODES_PREFIX, error_details)
        }
        _ => internal(ERROR_CODES_PREFIX),
    });

    let with_repos = warp::any().map(move || repos.clone());
    let with_conn = {
        let sync_conn = Arc::new(pool);
        warp::any().map(move || sync_conn.clone())
    };

    let fcm_uid = warp::header::<String>("X-Fcm-Uid");
    let user_addr = warp::header::<String>("X-User-Address");

    let device_unregister = warp::delete()
        .and(warp::path!("device"))
        .and(fcm_uid)
        .and(user_addr)
        .and(with_repos.clone())
        .and(with_conn.clone())
        .and_then(controllers::unregister_device);

    let device_update = warp::patch()
        .and(warp::path!("device"))
        .and(fcm_uid)
        .and(user_addr)
        .and(with_repos.clone())
        .and(with_conn.clone())
        .and(warp::body::json::<dto::UpdateDevice>())
        .and_then(controllers::update_device);

    let device_register = warp::put()
        .and(warp::path!("device"))
        .and(fcm_uid)
        .and(user_addr)
        .and(with_repos.clone())
        .and(with_conn.clone())
        .and(warp::body::json::<dto::NewDevice>())
        .and_then(controllers::register_device);

    let topic_unsubscribe = warp::delete()
        .and(warp::path!("topics"))
        .and(user_addr)
        .and(with_repos.clone())
        .and(with_conn.clone())
        .and(warp::body::json::<Option<dto::Topics>>())
        .and_then(controllers::unsubscribe_from_topics);

    let topic_subscribe = warp::post()
        .and(warp::path!("topics"))
        .and(user_addr)
        .and(with_repos.clone())
        .and(with_conn.clone())
        .and(warp::body::json::<dto::Topics>())
        .and_then(controllers::subscribe_to_topics);

    let log = warp::log::custom(access);

    info!("Starting push-notifications API server at 0.0.0.0:{}", port);

    let routes = device_unregister
        .or(device_update)
        .or(device_register)
        .or(topic_subscribe)
        .or(topic_unsubscribe)
        .recover(move |rej| {
            error!("{:?}", rej);
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
    use super::*;
    use crate::{device::FcmUid, subscription::SubscriptionMode};
    use chrono::FixedOffset;
    use warp::{http::StatusCode, Reply};

    pub async fn unregister_device(
        fcm_uid: FcmUid,
        addr: String,
        repos: Repos,
        pool: Pool,
    ) -> Result<impl Reply, Rejection> {
        repos
            .device
            .unregister(
                &Address::from_string(&addr).unwrap(),
                fcm_uid,
                &mut pool.get().await.unwrap(),
            )
            .await?;
        Ok(StatusCode::NO_CONTENT)
    }

    pub async fn register_device(
        fcm_uid: FcmUid,
        addr: String,
        repos: Repos,
        pool: Pool,
        device_info: dto::NewDevice,
    ) -> Result<impl Reply, Rejection> {
        let addr = Address::from_string(&addr).unwrap();
        let mut conn_lock = pool.get().await.unwrap();

        if repos
            .device
            .exists(&addr, fcm_uid.clone(), &mut conn_lock)
            .await?
        {
            return Ok(StatusCode::NO_CONTENT);
        }

        repos
            .device
            .register(
                &addr,
                fcm_uid,
                &device_info.lang.language,
                device_info.tz.utc_offset_seconds,
                &mut conn_lock,
            )
            .await?;
        Ok(StatusCode::CREATED)
    }

    pub async fn update_device(
        fcm_uid: FcmUid,
        addr: String,
        repos: Repos,
        pool: Pool,
        device_info: dto::UpdateDevice,
    ) -> Result<impl Reply, Rejection> {
        let response = if device_info.fcm.is_some() {
            StatusCode::OK
        } else {
            StatusCode::NO_CONTENT
        };
        repos
            .device
            .update(
                &Address::from_string(&addr).unwrap(),
                fcm_uid,
                device_info.lang.map(|l| l.language),
                device_info.tz.map(|tz| tz.utc_offset_seconds),
                device_info.fcm.map(|fcm| fcm.fcm_uid),
                &mut pool.get().await.unwrap(),
            )
            .await?;
        Ok(response)
    }

    pub async fn unsubscribe_from_topics(
        addr: String,
        repos: Repos,
        pool: Pool,
        topics: Option<dto::Topics>,
    ) -> Result<impl Reply, Rejection> {
        repos
            .subscriptions
            .unsubscribe(
                &Address::from_string(&addr).unwrap(),
                topics.map(|t| t.topics),
                &mut pool.get().await.unwrap(),
            )
            .await?;
        Ok(StatusCode::NO_CONTENT)
    }

    pub async fn subscribe_to_topics(
        addr: String,
        repos: Repos,
        pool: Pool,
        topics: dto::Topics,
    ) -> Result<impl Reply, Rejection> {
        repos
            .subscriptions
            .subscribe(
                &Address::from_string(&addr).unwrap(),
                topics.topics,
                SubscriptionMode::Repeat, //todo: find out the right value
                &mut pool.get().await.unwrap(),
            )
            .await?;
        Ok(StatusCode::NO_CONTENT)
    }
}

#[derive(Clone)]
pub struct Repos {
    pub device: device::Repo,
    pub subscriptions: subscription::Repo,
}

mod dto {
    use serde::Deserialize;

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

    #[derive(Deserialize)]
    pub struct Topics {
        pub topics: Vec<String>,
    }
}
