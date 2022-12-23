use diesel::{result::Error as DslError, ExpressionMethods, QueryDsl};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};

use crate::{
    model::{Address, AsBase58String, Lang},
    schema::{devices, subscribers},
    Error,
};

use crate::scoped_futures::ScopedFutureExt;

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
        tz_offset: i32,
        conn: &mut AsyncPgConnection,
    ) -> Result<(), Error> {
        conn.transaction(move |conn| {
            let address = address.as_base58_string();
            let lang = lang.to_string();

            async move {
                let device = (
                    devices::fcm_uid.eq(fcm_uid),
                    devices::subscriber_address.eq(&address),
                    devices::language.eq(lang),
                    devices::utc_offset_seconds.eq(tz_offset),
                );

                diesel::insert_into(subscribers::table)
                    .values(subscribers::address.eq(&address))
                    .execute(conn)
                    .await?;

                diesel::insert_into(devices::table)
                    .values(device)
                    .execute(conn)
                    .await?;

                Ok(())
            }
            .scope_boxed()
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
                diesel::delete(
                    devices::table
                        .filter(devices::subscriber_address.eq(&address))
                        .filter(devices::fcm_uid.eq(fcm_uid)),
                )
                .execute(conn)
                .await?;

                diesel::delete(subscribers::table.filter(subscribers::address.eq(&address)))
                    .execute(conn)
                    .await?;

                Ok(())
            }
            .scope_boxed()
        })
        .await
    }

    pub async fn exists(
        &self,
        address: &Address,
        conn: &mut AsyncPgConnection,
    ) -> Result<bool, Error> {
        let row_exists = devices::table
            .select(devices::fcm_uid)
            .filter(devices::subscriber_address.eq(address.as_base58_string()))
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
        tz_offset: Option<i32>,
        new_fcm_uid: Option<FcmUid>,
        conn: &mut AsyncPgConnection,
    ) -> Result<(), Error> {
        conn.transaction(move |conn| {
            let address = address.as_base58_string();
            let lang = lang.map(|l| l.to_string());

            async move {
                //TODO Performance issue: the following independent queries can be merged into one

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
                }

                if let Some(lang) = lang {
                    updater
                        .clone()
                        .set(devices::language.eq(lang))
                        .execute(conn)
                        .await?;
                }

                if let Some(tz) = tz_offset {
                    updater
                        .clone()
                        .set(devices::utc_offset_seconds.eq(tz))
                        .execute(conn)
                        .await?;
                }

                Ok(())
            }
            .scope_boxed()
        })
        .await
    }
}
