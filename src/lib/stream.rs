use crate::model::{Amount, Asset};

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum OrderType {
    Limit,
    Market,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Copy, Clone, PartialEq)]
pub enum OrderExecution {
    Full,
    Partial { percentage: f64 },
}

/// Raw price value with unknown decimals
pub type RawPrice = u64;

/// Price as integer together with corresponding decimals
pub struct PriceWithDecimals {
    pub price: u64,
    pub decimals: u8,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PriceOHLC {
    open: RawPrice,
    close: RawPrice,
    low: RawPrice,
    high: RawPrice,
}

pub enum Event {
    OrderExecuted {
        order_type: OrderType,
        side: OrderSide,
        amount_asset: Asset,
        price_asset: Asset,
        execution: OrderExecution,
    },
    PriceChanged {
        amount_asset: Asset,
        price_asset: Asset,
        current_price: PriceOHLC,
        previous_price: PriceOHLC,
    },
}

mod impls {
    use super::{PriceOHLC, PriceWithDecimals, RawPrice};
    use itertools::Itertools;
    use std::ops::Add;

    impl PriceWithDecimals {
        pub fn value(&self) -> f64 {
            let value = self.price as f64;
            let divisor = 10_f64.powi(self.decimals as i32);
            value / divisor
        }
    }

    #[test]
    fn test_price_decimals() {
        let p = |price, decimals| PriceWithDecimals { price, decimals };
        assert_eq!(p(12345678, 1).value(), 1234567.8);
        assert_eq!(p(12345678, 2).value(), 123456.78);
        assert_eq!(p(12345678, 3).value(), 12345.678);
        assert_eq!(p(12345678, 4).value(), 1234.5678);
    }

    impl PriceOHLC {
        pub fn from_single_value(price: RawPrice) -> Self {
            PriceOHLC {
                open: price,
                close: price,
                low: price,
                high: price,
            }
        }

        pub fn low_high(&self) -> (RawPrice, RawPrice) {
            debug_assert!(self.low <= self.high);
            (self.low, self.high)
        }

        pub fn has_crossed_threshold(&self, other: &Self, threshold: RawPrice) -> bool {
            debug_assert!(self.low <= self.high);
            debug_assert!(other.low <= other.high);
            let threshold = threshold as i64;
            let mut prices = [
                self.open as i64,
                self.close as i64,
                self.low as i64,
                self.high as i64,
                other.open as i64,
                other.close as i64,
                other.low as i64,
                other.high as i64,
            ];
            prices.sort_unstable_by(i64::cmp);
            prices
                .into_iter()
                .tuple_windows()
                .any(|(a, b)| i64::signum(threshold - a) != i64::signum(threshold - b))
        }
    }

    impl Add for &PriceOHLC {
        type Output = PriceOHLC;

        fn add(self, rhs: &PriceOHLC) -> PriceOHLC {
            debug_assert!(self.low <= self.high);
            debug_assert!(rhs.low <= rhs.high);
            debug_assert_eq!(self.close, rhs.open);
            PriceOHLC {
                open: self.open,
                close: rhs.close,
                low: self.low.min(rhs.low).min(rhs.open),
                high: self.high.max(rhs.high).max(rhs.open),
            }
        }
    }

    #[test]
    fn test_ohlc() {
        let ohlc = |open, low, high, close| PriceOHLC {
            open,
            close,
            low,
            high,
        };

        assert_eq!(ohlc(1, 0, 3, 2).low_high(), (0, 3));
        assert_eq!(&ohlc(1, 0, 3, 2) + &ohlc(2, 1, 9, 5), ohlc(1, 0, 9, 5));
        assert!(ohlc(1, 0, 3, 2).has_crossed_threshold(&ohlc(6, 5, 8, 7), 4));
    }
}
