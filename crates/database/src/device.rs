use diesel::{result::Error as DslError, AsChangeset, ExpressionMethods, QueryDsl};
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use model::{
    device::{Device, FcmUid, LocaleInfo},
    waves::{Address, AsBase58String},
};

use crate::{
    error::Error,
    schema::{devices, subscribers},
};

#[derive(Clone)]
pub struct Repo {}

impl Repo {
    pub async fn subscribers(
        &self,
        address: &Address,
        conn: &mut AsyncPgConnection,
    ) -> Result<Vec<Device>, Error> {
        let rows = devices::table
            .select((
                devices::uid,
                devices::fcm_uid,
                devices::language,
                devices::utc_offset_seconds,
            ))
            .filter(devices::subscriber_address.eq(address.as_base58_string()))
            .order(devices::uid)
            .load::<(i32, String, String, i32)>(conn)
            .await?;

        let devices = rows
            .into_iter()
            .map(|(device_uid, fcm_uid, lang, utc_offset_seconds)| Device {
                device_uid,
                fcm_uid,
                address: address.clone(),
                locale: LocaleInfo {
                    lang,
                    utc_offset_seconds,
                },
            })
            .collect();

        Ok(devices)
    }

    pub async fn register(
        &self,
        address: &Address,
        fcm_uid: &FcmUid,
        lang: &str,
        tz_offset: i32,
        conn: &mut AsyncPgConnection,
    ) -> Result<(), Error> {
        let address = address.as_base58_string();
        let lang = lang.to_string();

        let device = (
            devices::fcm_uid.eq(fcm_uid),
            devices::subscriber_address.eq(&address),
            devices::language.eq(lang),
            devices::utc_offset_seconds.eq(tz_offset),
        );

        diesel::insert_into(subscribers::table)
            .values(subscribers::address.eq(&address))
            .on_conflict_do_nothing()
            .execute(conn)
            .await?;

        diesel::insert_into(devices::table)
            .values(device)
            .execute(conn)
            .await?;

        Ok(())
    }

    pub async fn unregister(
        &self,
        address: &Address,
        fcm_uid: &FcmUid,
        conn: &mut AsyncPgConnection,
    ) -> Result<(), Error> {
        let address = address.as_base58_string();

        diesel::delete(
            devices::table
                .filter(devices::subscriber_address.eq(&address))
                .filter(devices::fcm_uid.eq(fcm_uid)),
        )
        .execute(conn)
        .await?;

        let extra_devices_with_same_addr = devices::table
            .select(devices::fcm_uid)
            .filter(devices::subscriber_address.eq(&address))
            .first::<FcmUid>(conn)
            .await;

        if optional(extra_devices_with_same_addr)?.is_none() {
            diesel::delete(subscribers::table.filter(subscribers::address.eq(&address)))
                .execute(conn)
                .await?;
        }

        Ok(())
    }

    pub async fn exists(
        &self,
        address: &Address,
        fcm_uid: &FcmUid,
        conn: &mut AsyncPgConnection,
    ) -> Result<bool, Error> {
        let row_exists = devices::table
            .select(devices::fcm_uid)
            .filter(devices::subscriber_address.eq(address.as_base58_string()))
            .filter(devices::fcm_uid.eq(fcm_uid))
            .first::<FcmUid>(conn)
            .await;

        Ok(optional(row_exists)?.is_some())
    }

    pub async fn update(
        &self,
        address: &Address,
        fcm_uid: &FcmUid,
        language: Option<String>,
        utc_offset_seconds: Option<i32>,
        new_fcm_uid: Option<FcmUid>,
        conn: &mut AsyncPgConnection,
    ) -> Result<(), Error> {
        #[derive(AsChangeset)]
        #[diesel(table_name = devices)]
        struct DeviceUpdate {
            language: Option<String>,
            utc_offset_seconds: Option<i32>,
            fcm_uid: Option<FcmUid>,
        }

        let updates = DeviceUpdate {
            language,
            utc_offset_seconds,
            fcm_uid: new_fcm_uid,
        };

        let address = address.as_base58_string();

        diesel::update(
            devices::table
                .filter(devices::fcm_uid.eq(fcm_uid))
                .filter(devices::subscriber_address.eq(address.clone())),
        )
        .set(&updates)
        .execute(conn)
        .await?;

        Ok(())
    }
}

fn optional<R>(query_result: Result<R, DslError>) -> Result<Option<R>, Error> {
    match query_result {
        Ok(r) => Ok(Some(r)),
        Err(DslError::NotFound) => Ok(None),
        Err(e) => Err(e.into()),
    }
}
