// use serde::{Deserialize, Deserializer};
// use std::convert::TryInto;
// use waves_rust::{api::Profile, util::Base58};

// #[derive(Deserialize)]
// pub struct Config {
//     #[serde(deserialize_with = "deserialize_profile")]
//     pub profile: Profile,
//     #[serde(deserialize_with = "deserialize_private_key")]
//     pub private_key: [u8; 32],
// }

// fn deserialize_profile<'de, D>(deserializer: D) -> Result<Profile, D::Error>
// where
//     D: Deserializer<'de>,
// {
//     let profile_str: String = Deserialize::deserialize(deserializer)?;
//     let profile = if profile_str.to_lowercase() == "mainnet" {
//         Profile::MAINNET
//     } else {
//         Profile::TESTNET
//     };
//     Ok(profile)
// }

// // todo error handling
// fn deserialize_private_key<'de, D>(deserializer: D) -> Result<[u8; 32], D::Error>
// where
//     D: Deserializer<'de>,
// {
//     let pk_base58: String = Deserialize::deserialize(deserializer)?;
//     let pk_bytes_vec =
//         Base58::decode(&pk_base58).expect("failed to decode private key from base58");
//     let pk_bytes: [u8; 32] = pk_bytes_vec.try_into().expect("Wrong private key length");
//     Ok(pk_bytes)
// }
