use crate::{
    error::Error,
    message::{LocalizedMessage, Message},
};

//TODO This is a low-level gateway to Localizer, can be private, no need to be public
struct RemoteGateway {}

impl RemoteGateway {
    pub async fn localize(message: &Message) -> Result<String, Error> {
        todo!()
    }
}

pub struct Repo {}

impl Repo {
    //TODO This is the factory method that can construct Repo during app startup.
    // Does all the network calls and caches everything (using the lower-level gateway above).
    // Can fail (this will lead to app crash on startup).
    pub fn new() -> Result<Self, Error> {
        todo!("localizer repo factory method impl")
    }

    //TODO This is the higher-level localization that is totally infallible and never does any network calls
    pub fn localize(&self, message: &Message) -> LocalizedMessage {
        //TODO parse input message, decide which localization keys and arguments we need
        match message {
            Message::OrderExecuted { .. } => { /* TODO */ }
            Message::PriceThresholdReached { .. } => { /* TODO */ }
        }
        todo!("localize message impl")
    }
}
