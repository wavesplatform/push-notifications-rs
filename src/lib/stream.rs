use std::fmt;

use crate::model::AssetPair;

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

/// Price value as floating point, decimals applied
pub type Price = f64;

/// Raw price value with unknown decimals
pub type RawPrice = u64;

/// Price as integer together with corresponding decimals
#[derive(Copy, Clone)]
pub struct PriceWithDecimals {
    pub price: u64,
    pub decimals: u8,
}

impl fmt::Debug for PriceWithDecimals {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}*10^-{}", self.price, self.decimals)
    }
}

/// Price range [low..high], stored as floating point numbers (decimals applied)
#[derive(Clone)]
pub struct PriceLowHigh {
    low: Price,
    high: Price,
}

impl fmt::Debug for PriceLowHigh {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}..{}]", self.low, self.high)
    }
}

pub enum Event {
    OrderExecuted {
        order_type: OrderType,
        side: OrderSide,
        asset_pair: AssetPair,
        execution: OrderExecution,
    },
    PriceChanged {
        asset_pair: AssetPair,
        price_range: PriceLowHigh,
    },
}

mod impls {
    use super::{Price, PriceLowHigh, PriceWithDecimals};

    impl PriceWithDecimals {
        pub fn value(&self) -> Price {
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

    impl PriceLowHigh {
        pub fn from_single_price(price: PriceWithDecimals) -> Self {
            let value = price.value();
            PriceLowHigh {
                low: value,
                high: value,
            }
        }

        pub fn merge(self, price: PriceWithDecimals) -> Self {
            debug_assert!(self.low <= self.high, "broken invariant low <= high");
            let price = price.value();
            PriceLowHigh {
                low: if price < self.low { price } else { self.low },
                high: if price > self.high { price } else { self.high },
            }
        }

        pub fn is_empty(&self) -> bool {
            self.low == self.high
        }

        pub fn low_high(&self) -> (Price, Price) {
            debug_assert!(self.low <= self.high, "broken invariant low <= high");
            (self.low, self.high)
        }

        pub fn contains(&self, price: Price) -> bool {
            debug_assert!(self.low <= self.high, "broken invariant low <= high");
            self.low <= price && price <= self.high
        }
    }

    impl PriceWithDecimals {
        pub fn merge(self, other: Self) -> PriceLowHigh {
            PriceLowHigh::from_single_price(self).merge(other)
        }
    }

    #[test]
    fn test_price_low_high() {
        let p = |price, decimals| PriceWithDecimals { price, decimals };

        let p1 = PriceLowHigh::from_single_price(p(12345, 2));
        assert_eq!(p1.is_empty(), true);
        assert_eq!(p1.low_high(), (123.45, 123.45));
        assert_eq!(p1.contains(123.44), false);
        assert_eq!(p1.contains(123.46), false);

        let p2 = p1.merge(p(12000, 2));
        assert_eq!(p2.is_empty(), false);
        assert_eq!(p2.low_high(), (120.00, 123.45));
        assert_eq!(p2.contains(123.00), true);
        assert_eq!(p2.contains(100.00), false);
        assert_eq!(p2.contains(200.00), false);

        let p3 = p2.merge(p(54321, 2));
        assert_eq!(p3.is_empty(), false);
        assert_eq!(p3.low_high(), (120.00, 543.21));
        assert_eq!(p3.contains(123.00), true);
        assert_eq!(p3.contains(100.00), false);
        assert_eq!(p3.contains(600.00), false);

        let p4 = p3.merge(p(7000000, 4)); // different decimals - ok
        assert_eq!(p4.is_empty(), false);
        assert_eq!(p4.low_high(), (120.00, 700.00));
    }
}
