use crate::waves::Address;

pub type FcmUid = String;

#[derive(Debug)]
pub struct Device {
    pub device_uid: i32,
    pub address: Address,
    pub fcm_uid: FcmUid,
    pub locale: LocaleInfo,
}

#[derive(Debug)]
pub struct LocaleInfo {
    pub lang: Lang,
    pub utc_offset_seconds: i32,
}

pub type Lang = String;
