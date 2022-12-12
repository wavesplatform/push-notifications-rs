use chrono::{DateTime, FixedOffset, Utc};
use diesel::{dsl::exists, result::Error as DslError, ExpressionMethods, QueryDsl};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use futures::FutureExt;

use crate::{
    model::{Address, AsBase58String, Lang},
    schema::{devices, subscribers},
    Error,
};

pub type FcmUid = String;

pub struct Device {
    pub device_uid: i32,
    pub address: Address,
    pub fcm_uid: FcmUid,
    pub lang: Lang,
}

#[derive(Clone)]
pub struct Repo {}

impl Repo {
    pub async fn subscribers(
        &self,
        address: &Address,
        conn: &mut AsyncPgConnection,
    ) -> Result<Vec<Device>, Error> {
        let rows = devices::table
            .select((devices::uid, devices::fcm_uid, devices::language))
            .filter(devices::subscriber_address.eq(address.as_base58_string()))
            .order(devices::uid)
            .load::<(i32, String, String)>(conn)
            .await?;

        let devices = rows
            .into_iter()
            .map(|(device_uid, fcm_uid, lang)| Device {
                device_uid,
                fcm_uid,
                address: address.clone(),
                lang,
            })
            .collect();

        Ok(devices)
    }

    pub async fn register(
        &self,
        address: &Address,
        fcm_uid: FcmUid,
        lang: &str,
        tz: FixedOffset,
        conn: &mut AsyncPgConnection,
    ) -> Result<(), Error> {
        conn.transaction(move |conn| {
            let address = address.as_base58_string();
            let timestamptz = Utc::now().with_timezone(&tz);
            let lang = lang.to_string();

            async move {
                let subscriber = (
                    subscribers::created_at.eq(timestamptz),
                    subscribers::updated_at.eq(timestamptz),
                    subscribers::address.eq(address.clone()),
                );

                diesel::insert_into(subscribers::table)
                    .values(subscriber)
                    .execute(conn)
                    .await?;

                let device = (
                    devices::created_at.eq(timestamptz),
                    devices::updated_at.eq(timestamptz),
                    devices::fcm_uid.eq(fcm_uid),
                    devices::subscriber_address.eq(address),
                    devices::language.eq(lang),
                );

                diesel::insert_into(devices::table)
                    .values(device)
                    .execute(conn)
                    .await?;

                Ok(())
            }
            .boxed()
        })
        .await
    }

    pub async fn unregister(
        &self,
        address: &Address,
        fcm_uid: FcmUid,
        conn: &mut AsyncPgConnection,
    ) -> Result<(), Error> {
        conn.transaction(move |conn| {
            let address = address.as_base58_string();
            async move {
                diesel::delete(subscribers::table.filter(subscribers::address.eq(address.clone())))
                    .execute(conn)
                    .await?;

                diesel::delete(
                    devices::table
                        .filter(devices::fcm_uid.eq(fcm_uid))
                        .filter(devices::subscriber_address.eq(address)),
                )
                .execute(conn)
                .await?;

                Ok(())
            }
            .boxed()
        })
        .await
    }

    pub async fn exists(
        &self,
        address: &Address,
        fcm_uid: FcmUid,
        conn: &mut AsyncPgConnection,
    ) -> Result<bool, Error> {
        let row_exists = devices::table
            .select(devices::fcm_uid)
            .filter(devices::fcm_uid.eq(fcm_uid))
            .first::<FcmUid>(conn)
            .await;

        match row_exists {
            Ok(_) => Ok(true),
            Err(DslError::NotFound) => Ok(false), // no .optional() in async diesel?
            Err(e) => Err(e.into()),
        }
    }

    pub async fn update(
        &self,
        address: &Address,
        fcm_uid: FcmUid,
        lang: Option<String>,
        tz: Option<FixedOffset>,
        new_fcm_uid: Option<FcmUid>,
        conn: &mut AsyncPgConnection,
    ) -> Result<(), Error> {
        conn.transaction(move |conn| {
            let address = address.as_base58_string();
            let lang = lang.map(|l| l.to_string());

            async move {
                // refactor asap
                let updater = diesel::update(
                    devices::table
                        .filter(devices::fcm_uid.eq(fcm_uid.clone()))
                        .filter(devices::subscriber_address.eq(address.clone())),
                );

                if let Some(new_fcm_uid) = new_fcm_uid {
                    updater
                        .clone()
                        .set(devices::fcm_uid.eq(new_fcm_uid))
                        .execute(conn)
                        .await?;

                    return Ok(());
                }

                if let Some(lang) = lang {
                    updater
                        .clone()
                        .set(devices::language.eq(lang))
                        .execute(conn)
                        .await?;
                }

                if let Some(tz) = tz {
                    let (created_at, updated_at) = devices::table
                        .select((devices::created_at, devices::updated_at))
                        .filter(devices::fcm_uid.eq(fcm_uid))
                        .filter(devices::subscriber_address.eq(address.clone()))
                        .get_result::<(DateTime<Utc>, DateTime<Utc>)>(conn)
                        .await?;

                    let (created_at, updated_at) =
                        (created_at.with_timezone(&tz), updated_at.with_timezone(&tz));

                    updater
                        .set((
                            devices::created_at.eq(created_at),
                            devices::updated_at.eq(updated_at),
                        ))
                        .execute(conn)
                        .await?;

                    diesel::update(
                        subscribers::table.filter(subscribers::address.eq(address.clone())),
                    )
                    .set((
                        subscribers::created_at.eq(created_at),
                        subscribers::updated_at.eq(updated_at),
                    ))
                    .execute(conn)
                    .await?;
                }

                Ok(())
            }
            .boxed()
        })
        .await
    }
}
