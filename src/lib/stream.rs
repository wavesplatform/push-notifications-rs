use std::fmt;

use crate::model::AssetPair;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum OrderType {
    Limit,
    Market,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Copy, Clone, PartialEq, Debug)]
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

/// Price range stored as floating point numbers (decimals applied).
/// Each bound (upper and lower) can be either excluded or included,
/// which affects checking whether a price lies inside or outside the range.
/// That said, four options are possible:
/// `[low..high]`, `(low..high)`, `[low..high)` and `(low..high]`.
#[derive(Clone, Default)]
pub struct PriceRange {
    low: Bound<Price>,
    high: Bound<Price>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Bound<T> {
    #[default]
    None,
    Included(T),
    Excluded(T),
}

impl fmt::Debug for PriceRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.low, self.high) {
            (Bound::None, _) | (_, Bound::None) => write!(f, "[empty]"),
            (Bound::Included(low), Bound::Included(high)) => write!(f, "[{}..{}]", low, high),
            (Bound::Included(low), Bound::Excluded(high)) => write!(f, "[{}..{})", low, high),
            (Bound::Excluded(low), Bound::Included(high)) => write!(f, "({}..{}]", low, high),
            (Bound::Excluded(low), Bound::Excluded(high)) => write!(f, "({}..{})", low, high),
        }
    }
}

#[derive(Debug)]
pub enum Event {
    OrderExecuted {
        order_type: OrderType,
        side: OrderSide,
        asset_pair: AssetPair,
        execution: OrderExecution,
        timestamp: i64,
    },
    PriceChanged {
        asset_pair: AssetPair,
        price_range: PriceRange,
        timestamp: i64,
    },
}

mod impls {
    use super::{Bound, Price, PriceRange, PriceWithDecimals};

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

    impl<T: Default + Copy> Bound<T> {
        fn value(&self) -> T {
            match self {
                Bound::None => T::default(),
                Bound::Included(value) | Bound::Excluded(value) => *value,
            }
        }
    }

    impl PriceRange {
        /// Crete new empty price range, no price is considered inside it.
        pub fn empty() -> Self {
            PriceRange {
                low: Bound::None,
                high: Bound::None,
            }
        }

        /// Whether this price range is empty.
        pub fn is_empty(&self) -> bool {
            // If no bounds set the range is empty
            if self.low == Bound::None && self.high == Bound::None {
                return true;
            }
            // If at least one bound is inclusive the range is not empty
            if matches!(self.low, Bound::Included(_)) || matches!(self.high, Bound::Included(_)) {
                return false;
            }
            // If both bounds are exclusive, the range is non-empty if bounds are non-equal
            self.low == self.high
        }

        /// Get low and high bounds of the range,
        /// without the information whether these bounds inclusive or exclusive.
        /// Panics if the range is empty.
        pub fn low_high(&self) -> (Price, Price) {
            debug_assert!(self.low.value() <= self.high.value(), "low <= high");
            assert!(!self.is_empty(), "range is empty");
            (self.low.value(), self.high.value())
        }

        /// Check if the given price is withing the range.
        pub fn contains(&self, price: Price) -> bool {
            debug_assert!(self.low.value() <= self.high.value(), "low <= high");
            match (self.low, self.high) {
                (Bound::None, _) | (_, Bound::None) => false,
                (Bound::Included(low), Bound::Included(high)) => low <= price && price <= high,
                (Bound::Included(low), Bound::Excluded(high)) => low <= price && price < high,
                (Bound::Excluded(low), Bound::Included(high)) => low < price && price <= high,
                (Bound::Excluded(low), Bound::Excluded(high)) => low < price && price < high,
            }
        }

        /// Extend the range by adding a price to it.
        pub fn extend(self, price: Price) -> Self {
            debug_assert!(self.low.value() <= self.high.value(), "low <= high");
            let price_included = Bound::Included(price);
            PriceRange {
                low: if self.low == Bound::None || price < self.low.value() {
                    price_included
                } else {
                    self.low
                },
                high: if self.low == Bound::None || price > self.high.value() {
                    price_included
                } else {
                    self.high
                },
            }
        }

        /// Exclude from the range bounds that equals to the given price.
        pub fn exclude_bound(self, price: Price) -> Self {
            PriceRange {
                low: if self.low == Bound::Included(price) {
                    Bound::Excluded(price)
                } else {
                    self.low
                },
                high: if self.high == Bound::Included(price) {
                    Bound::Excluded(price)
                } else {
                    self.high
                },
            }
        }
    }

    #[test] #[rustfmt::skip]
    fn test_price_range_is_empty() {
        assert_eq!(PriceRange::empty().is_empty(), true);
        assert_eq!(PriceRange::empty().exclude_bound(0.0).is_empty(), true);
        assert_eq!(PriceRange::empty().exclude_bound(1.0).is_empty(), true);
        assert_eq!(PriceRange::empty().extend(1.0).is_empty(), false);
        assert_eq!(PriceRange::empty().extend(1.0).exclude_bound(1.0).is_empty(), true);
        assert_eq!(PriceRange::empty().extend(1.0).extend(2.0).is_empty(), false);
        assert_eq!(PriceRange::empty().extend(1.0).exclude_bound(2.0).is_empty(), false);
        assert_eq!(PriceRange::empty().exclude_bound(1.0).extend(1.0).is_empty(), false);

        assert!(PriceRange::default().is_empty());
    }

    #[test]
    fn test_price_range_contains() {
        let p = PriceRange::empty();
        assert_eq!(p.is_empty(), true);
        assert_eq!(p.contains(0.0), false);

        let p = PriceRange::empty().exclude_bound(42.0);
        assert_eq!(p.is_empty(), true);
        assert_eq!(p.contains(0.0), false);
        assert_eq!(p.contains(42.0), false);

        let p = PriceRange::empty().extend(42.0);
        assert_eq!(p.is_empty(), false);
        assert_eq!(p.contains(0.0), false);
        assert_eq!(p.contains(42.0), true);
        assert_eq!(p.contains(41.9), false);
        assert_eq!(p.contains(42.1), false);
        assert_eq!(p.low_high(), (42.0, 42.0));

        let p = PriceRange::empty().extend(42.0).exclude_bound(42.0);
        assert_eq!(p.is_empty(), true);
        assert_eq!(p.contains(0.0), false);
        assert_eq!(p.contains(42.0), false);

        let p = PriceRange::empty().extend(123.45).extend(120.00);
        assert_eq!(p.low_high(), (120.00, 123.45));
        assert_eq!(p.contains(120.00), true);
        assert_eq!(p.contains(123.00), true);
        assert_eq!(p.contains(123.45), true);
        assert_eq!(p.contains(100.00), false);
        assert_eq!(p.contains(200.00), false);

        let p = PriceRange::empty()
            .extend(3.0)
            .extend(1.0)
            .extend(2.0)
            .exclude_bound(1.0)
            .exclude_bound(2.0);
        assert_eq!(p.low_high(), (1.0, 3.0));
        assert_eq!(p.contains(1.0), false);
        assert_eq!(p.contains(2.0), true);
        assert_eq!(p.contains(3.0), true);

        let p = PriceRange::empty()
            .extend(3.0)
            .extend(1.0)
            .extend(2.0)
            .exclude_bound(3.0)
            .exclude_bound(5.0);
        assert_eq!(p.low_high(), (1.0, 3.0));
        assert_eq!(p.contains(1.0), true);
        assert_eq!(p.contains(2.0), true);
        assert_eq!(p.contains(3.0), false);
        assert_eq!(p.contains(5.0), false);
    }
}
