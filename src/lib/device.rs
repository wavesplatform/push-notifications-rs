use diesel_async::AsyncPgConnection;

use crate::{
    model::{Address, Lang},
    Error,
};

pub type FcmUid = String;

pub struct Device {
    pub device_uid: u64,
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
        todo!("impl get devices by address")
    }
}
