use lib::config::Config;
use lib::Error;
use warp::{Filter, Rejection};
use wavesexchange_log::{error, info};
use wavesexchange_warp::error::{
    error_handler_with_serde_qs, handler, internal, not_found, validation,
};
use wavesexchange_warp::log::access;
use wavesexchange_warp::MetricsWarpBuilder;

const ERROR_CODES_PREFIX: u16 = 95;

pub async fn start(port: u16, metrics_port: u16) {
    let error_handler = handler(ERROR_CODES_PREFIX, |err| match err {
        // Error::ValidationError(field, error_details) => {
        //     let mut error_details = error_details.to_owned();
        //     if let Some(details) = error_details.as_mut() {
        //         details.insert("parameter".to_owned(), field.to_owned());
        //     }
        //     validation::invalid_parameter(ERROR_CODES_PREFIX, error_details)
        // }
        _ => internal(ERROR_CODES_PREFIX),
    });

    let fcm_uid = warp::header::<String>("X-Fcm-Uid");
    let auth = warp::header::<String>("Authorization");

    let device_delete = warp::delete()
        .and(warp::path!("device"))
        .and(fcm_uid)
        .and(auth)
        .and_then(|_, _| async { Ok::<_, Rejection>("ok") });

    let device_edit = warp::patch()
        .and(warp::path!("device"))
        .and(fcm_uid)
        .and(auth)
        .and_then(|_, _| async { Ok::<_, Rejection>("ok") });

    let device_new = warp::put()
        .and(warp::path!("device"))
        .and(fcm_uid)
        .and(auth)
        .and_then(|_, _| async { Ok::<_, Rejection>("ok") });

    let topic_unsubscribe = warp::delete()
        .and(warp::path!("device"))
        .and(fcm_uid)
        .and(auth)
        .and_then(|_, _| async { Ok::<_, Rejection>("ok") });

    let topic_subscribe = warp::post()
        .and(warp::path!("device"))
        .and(fcm_uid)
        .and(auth)
        .and_then(|_, _| async { Ok::<_, Rejection>("ok") });

    let log = warp::log::custom(access);

    info!("Starting push-notifications API server at 0.0.0.0:{}", port);

    let routes = device_delete
        .or(device_edit)
        .or(device_new)
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

#[tokio::main]
async fn main() -> Result<(), Error> {
    let config = Config::load()?;

    info!(
        "Starting push-notifications api service with config: {:?}",
        config
    );

    start(config.api.port, config.api.metrics_port).await;

    Ok(())
}
