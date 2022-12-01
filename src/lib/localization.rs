use crate::{error::Error, message::Message};

pub struct RemoteGateway {}

impl RemoteGateway {
    pub async fn localize(message: &Message) -> Result<String, Error> {
        todo!()
    }
}
