#[tokio::main]
async fn main() {
    println!("Sender started");
}

// use chrono::{DateTime, Utc};
// use waves_rust::model::{Address, Amount, AssetId, Id};

// // INPUTS:
// struct Subscription {
//     subscriber: Address,
//     created_at: DateTime<Utc>,
//     topic: Topic,
// }

// enum Topic {
//     OrderFulfilled {
//         amount_asset: Option<AssetId>,
//         price_asset: Option<AssetId>,
//     },
//     PriceThreshold {
//         amount_asset: Option<AssetId>,
//         price_asset: Option<AssetId>,
//         price_threshold: Amount,
//         once: bool,
//     },
// }

// // DERIVED
// struct Message {
//     // todo key

//     // trigger: Trigger,
// }

// // struct Trigger {
// //     timestamp: DateTime<Utc>,
// //     transaction_id: Id,
// //     height: u32,
// // }
// let srr: StreamReadReply = xreadgroup(
//     stream_name,
//     group_name,
//     consumer_name,
//     batch_max_size.to_owned(),
//     ">",
// )
// .query_async(&mut con)
// .await?;

// info!("got the next processes batch");

// for StreamKey { key: _, ids } in srr.keys {
//     for StreamId { id, map } in ids {
//         for (_, v) in map {
//             // ...
//             on_process_cb(process)?;

//             xack(stream_name, group_name, &id)
//                 .query_async(&mut con)
//                 .await?;

//             xdel(stream_name, &id).query_async(&mut con).await?;
