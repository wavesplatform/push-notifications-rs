use crate::error::Error;

pub struct RemoteGateway {}

impl RemoteGateway {
    pub async fn send(
        localized_text: impl Into<String>,
        fcm_uid: impl Into<String>,
    ) -> Result<(), Error> {
        todo!("impl")
    }
}
