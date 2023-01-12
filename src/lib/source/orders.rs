//! Source of Order events

#[allow(dead_code)] // TODO Due to unused fields in JSON structs. Cleanup later.
mod json {
    use bigdecimal::BigDecimal;
    use serde::Deserialize;

    #[derive(Deserialize, Debug, Clone)]
    struct Envelope {
        /// The type of the message: 'osu'.
        #[serde(rename = "T")]
        msg_type: MessageType,

        /// Unix timestamp of this message in milliseconds. It is a Matcher's timestamp.
        /// This parameter is internal to the Matcher and is not recommended to use.
        #[serde(rename = "_")]
        timestamp: i64,

        /// Updated orders
        #[serde(rename = "o")] // o = [o]rders
        data: Vec<OrderUpdate>,
    }

    #[non_exhaustive] // The Redis feed only supports "osu" variant as ow now, but other feeds (websockets) supports more.
    #[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
    enum MessageType {
        #[serde(rename = "osu")] // osu = [o]rder[s] [u]pdated
        OrdersUpdated,
    }

    #[derive(Deserialize, Debug, Clone)]
    struct OrderUpdate {
        /// The order's id
        #[serde(rename = "i")]
        order_id: String,

        /// The address of order's owner
        #[serde(rename = "o")]
        owner_address: String,

        /// The specified order timestamp
        #[serde(rename = "t")]
        order_timestamp: i64,

        /// The amount asset
        #[serde(rename = "A")]
        amount_asset: String,

        /// The price asset
        #[serde(rename = "P")]
        price_asset: String,

        /// The order side: BUY | SELL | buy | sell.
        /// Lowercase variants will be in the future versions (2.2.0+). Please support all variants!
        #[serde(rename = "S")]
        side: OrderSide, // buy | sell

        /// The order type: LIMIT | MARKET | limit | market.
        /// Lowercase variants will be in the future versions (2.2.0+). Please support all variants!
        #[serde(rename = "T")]
        order_type: OrderType, // limit | market

        /// The specified order's price
        #[serde(rename = "p")]
        price: BigDecimal,

        /// The total order's amount
        #[serde(rename = "a")]
        amount: BigDecimal,

        /// The specified order's fee
        #[serde(rename = "f")]
        fee: BigDecimal,

        /// The fee asset
        #[serde(rename = "F")]
        fee_asset: String,

        /// The order status: FILLED | Filled | PARTIALLY_FILLED | PartiallyFilled | CANCELLED | Cancelled.
        /// Uppercase variants will be removed in the future versions (2.2.0+). Please support all variants!
        #[serde(rename = "s")]
        status: OrderStatus, // Filled | PartiallyFilled | Cancelled

        /// The current filled amount, including this and all previous matches
        #[serde(rename = "q")]
        filled_amount_accumulated: BigDecimal,

        /// The current filled fee, including this and all previous matches
        #[serde(rename = "Q")]
        filled_fee_accumulated: BigDecimal,

        /// The average filled price among all trades with maximum priceDecimals digits after point.
        /// Can be computed as `average weighed price = E / q`.
        /// This field will be removed in version 2.2.3
        #[serde(rename = "r")]
        avg_filled_price: Option<BigDecimal>,

        /// The update event timestamp. It is a Matcher's (almost) timestamp
        #[serde(rename = "Z")]
        event_timestamp: i64,

        /// The executed amount during this match, if it is Filled or PartiallyFilled event
        #[serde(rename = "c")]
        executed_amount: Option<BigDecimal>,

        /// The executed fee during this match, if it is Filled or PartiallyFilled event
        #[serde(rename = "h")]
        executed_fee: Option<BigDecimal>,

        /// The execution price, if it is Filled or PartiallyFilled event
        #[serde(rename = "e")]
        executed_price: Option<BigDecimal>,

        /// Total executed price assets. Will be available in version 2.2.3
        #[serde(rename = "E")]
        total_executed_price_assets: Option<BigDecimal>,
    }

    #[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
    enum OrderSide {
        #[serde(rename = "buy")]
        Buy,

        #[serde(rename = "sell")]
        Sell,
    }

    #[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
    enum OrderType {
        #[serde(rename = "limit")]
        Limit,

        #[serde(rename = "market")]
        Market,
    }

    #[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
    enum OrderStatus {
        #[serde(rename = "Filled")]
        Filled,

        #[serde(rename = "PartiallyFilled")]
        PartiallyFilled,

        #[serde(rename = "Cancelled")]
        Cancelled,
    }

    #[rustfmt::skip]
    #[test]
    fn test_orders_deserialize() -> anyhow::Result<()> {
        use serde_json::json;
        use std::str::FromStr;

        let big = |x: f64| -> BigDecimal { BigDecimal::from_str(x.to_string().as_str()).unwrap() };

        let check_avg_price = |order: &OrderUpdate| {
            if let (Some(avg_filled_price), Some(total_executed_price_assets)) = (&order.avg_filled_price, &order.total_executed_price_assets) {
                let avg_price = total_executed_price_assets / &order.filled_amount_accumulated;
                assert_eq!(avg_price, *avg_filled_price, "order fields disagree");
            }
        };

        let order_cancel = json!(
            {
              "T" : "osu",
              "_" : 1673428863604_i64,
              "o" : [ {
                "i" : "JX4G8f5ehPyUPfH12DRevvjCGSP7LaRcy9ToddLdqKL",
                "o" : "3Q6pToUA28zJbMJUfB5xoGgfqqni11H7NPq",
                "t" : 1673428862971_i64,
                "A" : "WAVES",
                "P" : "GwT5y18jcrrppAuj5VkfnHLG8WRf3TNzmhREQkY4pzd8",
                "S" : "sell",
                "T" : "limit",
                "p" : "5.0",
                "a" : "1.0",
                "f" : "0.003",
                "F" : "WAVES",
                "s" : "Cancelled",
                "q" : "0.0",
                "Q" : "0.0",
                "r" : "0.0",
                "Z" : 1673428862976_i64
              } ]
            }
        );
        let envelope = serde_json::from_value::<Envelope>(order_cancel)?;
        assert_eq!(envelope.msg_type, MessageType::OrdersUpdated);
        assert_eq!(envelope.timestamp, 1673428863604);
        assert_eq!(envelope.data.len(), 1);

        let order = &envelope.data[0];
        {
            assert_eq!(order.order_id, "JX4G8f5ehPyUPfH12DRevvjCGSP7LaRcy9ToddLdqKL");
            assert_eq!(order.owner_address, "3Q6pToUA28zJbMJUfB5xoGgfqqni11H7NPq");
            assert_eq!(order.order_timestamp, 1673428862971);
            assert_eq!(order.amount_asset, "WAVES");
            assert_eq!(order.price_asset, "GwT5y18jcrrppAuj5VkfnHLG8WRf3TNzmhREQkY4pzd8");
            assert_eq!(order.side, OrderSide::Sell);
            assert_eq!(order.order_type, OrderType::Limit);
            assert_eq!(order.price, big(5.0));
            assert_eq!(order.amount, big(1.0));
            assert_eq!(order.fee, big(0.003));
            assert_eq!(order.fee_asset, "WAVES");
            assert_eq!(order.status, OrderStatus::Cancelled);
            assert_eq!(order.filled_amount_accumulated, big(0.0));
            assert_eq!(order.filled_fee_accumulated, big(0.0));
            assert_eq!(order.avg_filled_price, Some(big(0.0)));
            assert_eq!(order.event_timestamp, 1673428862976);
            assert_eq!(order.executed_amount, None);
            assert_eq!(order.executed_fee, None);
            assert_eq!(order.executed_price, None);
            assert_eq!(order.total_executed_price_assets, None);
            check_avg_price(order);
        }

        let order_fill = json!(
            {
              "T" : "osu",
              "_" : 1673428865504_i64,
              "o" : [ {
                "i" : "DbGrYjRnRazkajgYHpekfB72EHBmmQjVPrgpLSJb3MTq",
                "o" : "3Q6pToUA28zJbMJUfB5xoGgfqqni11H7NPq",
                "t" : 1673428865872_i64,
                "A" : "WAVES",
                "P" : "GwT5y18jcrrppAuj5VkfnHLG8WRf3TNzmhREQkY4pzd8",
                "S" : "buy",
                "T" : "limit",
                "p" : "5.0",
                "a" : "1.0",
                "f" : "0.003",
                "F" : "WAVES",
                "s" : "Filled",
                "q" : "1.0",
                "Q" : "0.003",
                "r" : "5.0",
                "Z" : 1673428865504_i64,
                "c" : "1.0",
                "h" : "0.003",
                "e" : "5.0",
                "E" : "5.0"
              }, {
                "i" : "GR6WbwBxs6q8MXqLaz8a53epGKuqxBaM8fF9RDD5NiLW",
                "o" : "3Q6ujVDbX57oLsXxifqfTcycgb4S8U3DLFz",
                "t" : 1673428865875_i64,
                "A" : "WAVES",
                "P" : "GwT5y18jcrrppAuj5VkfnHLG8WRf3TNzmhREQkY4pzd8",
                "S" : "sell",
                "T" : "limit",
                "p" : "5.0",
                "a" : "5.0",
                "f" : "0.003",
                "F" : "WAVES",
                "s" : "PartiallyFilled",
                "q" : "1.0",
                "Q" : "6.0E-4",
                "r" : "5.0",
                "Z" : 1673428865504_i64,
                "c" : "1.0",
                "h" : "6.0E-4",
                "e" : "5.0",
                "E" : "5.0"
              } ]
            }
        );
        let envelope = serde_json::from_value::<Envelope>(order_fill)?;
        assert_eq!(envelope.msg_type, MessageType::OrdersUpdated);
        assert_eq!(envelope.timestamp, 1673428865504);
        assert_eq!(envelope.data.len(), 2);

        let order = &envelope.data[0];
        {
            assert_eq!(order.order_id, "DbGrYjRnRazkajgYHpekfB72EHBmmQjVPrgpLSJb3MTq");
            assert_eq!(order.owner_address, "3Q6pToUA28zJbMJUfB5xoGgfqqni11H7NPq");
            assert_eq!(order.order_timestamp, 1673428865872);
            assert_eq!(order.amount_asset, "WAVES");
            assert_eq!(order.price_asset, "GwT5y18jcrrppAuj5VkfnHLG8WRf3TNzmhREQkY4pzd8");
            assert_eq!(order.side, OrderSide::Buy);
            assert_eq!(order.order_type, OrderType::Limit);
            assert_eq!(order.price, big(5.0));
            assert_eq!(order.amount, big(1.0));
            assert_eq!(order.fee, big(0.003));
            assert_eq!(order.fee_asset, "WAVES");
            assert_eq!(order.status, OrderStatus::Filled);
            assert_eq!(order.filled_amount_accumulated, big(1.0));
            assert_eq!(order.filled_fee_accumulated, big(0.003));
            assert_eq!(order.avg_filled_price, Some(big(5.0)));
            assert_eq!(order.event_timestamp, 1673428865504);
            assert_eq!(order.executed_amount, Some(big(1.0)));
            assert_eq!(order.executed_fee, Some(big(0.003)));
            assert_eq!(order.executed_price, Some(big(5.0)));
            assert_eq!(order.total_executed_price_assets, Some(big(5.0)));
            check_avg_price(order);
        }

        let order = &envelope.data[1];
        {
            assert_eq!(order.order_id, "GR6WbwBxs6q8MXqLaz8a53epGKuqxBaM8fF9RDD5NiLW");
            assert_eq!(order.owner_address, "3Q6ujVDbX57oLsXxifqfTcycgb4S8U3DLFz");
            assert_eq!(order.order_timestamp, 1673428865875);
            assert_eq!(order.amount_asset, "WAVES");
            assert_eq!(order.price_asset, "GwT5y18jcrrppAuj5VkfnHLG8WRf3TNzmhREQkY4pzd8");
            assert_eq!(order.side, OrderSide::Sell);
            assert_eq!(order.order_type, OrderType::Limit);
            assert_eq!(order.price, big(5.0));
            assert_eq!(order.amount, big(5.0));
            assert_eq!(order.fee, big(0.003));
            assert_eq!(order.fee_asset, "WAVES");
            assert_eq!(order.status, OrderStatus::PartiallyFilled);
            assert_eq!(order.filled_amount_accumulated, big(1.0));
            assert_eq!(order.filled_fee_accumulated, big(0.0006));
            assert_eq!(order.avg_filled_price, Some(big(5.0)));
            assert_eq!(order.event_timestamp, 1673428865504);
            assert_eq!(order.executed_amount, Some(big(1.0)));
            assert_eq!(order.executed_fee, Some(big(0.0006)));
            assert_eq!(order.executed_price, Some(big(5.0)));
            assert_eq!(order.total_executed_price_assets, Some(big(5.0)));
            check_avg_price(order);
        }

        Ok(())
    }
}
