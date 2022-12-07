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
        let address = address.as_base58_string();

        let rows = devices::table
            .select((
                devices::uid,
                devices::fcm_uid,
                devices::subscriber_address,
                devices::language,
            ))
            .filter(devices::subscriber_address.eq(address))
            .order(devices::uid)
            .load::<(i32, String, String, String)>(conn)
            .await?;

        let devices = rows
            .into_iter()
            .map(|(device_uid, fcm_uid, address, lang)| Device {
                device_uid,
                fcm_uid,
                address: Address::from_string(&address).expect("address in db"),
                lang,
            })
            .collect();

        Ok(devices)
    }
}
