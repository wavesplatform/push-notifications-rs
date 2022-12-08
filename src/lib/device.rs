use diesel::{ExpressionMethods, QueryDsl};
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use crate::{
    model::{Address, AsBase58String, Lang},
    schema::devices,
    Error,
};

pub type FcmUid = String;

pub struct Device {
    pub device_uid: i32,
    pub address: Address,
    pub fcm_uid: FcmUid,
    pub lang: Lang,
}

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
}
