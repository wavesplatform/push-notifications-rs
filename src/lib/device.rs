use waves_rust::model::Address;

pub type FcmUid = String;

pub struct Repo {}

impl Repo {
    pub async fn subscribers(address: &Address) -> Vec<FcmUid> {
        todo!("impl")
        // vec![]
    }
}
