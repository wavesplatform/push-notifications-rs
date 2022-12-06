use chrono::Duration;

pub fn exponential(initial_interval: &Duration, multiplier: f32, attempts_count: u8) -> Duration {
    *initial_interval * multiplier.powf(attempts_count as f32) as i32
}

#[cfg(test)]
mod tests {
    #[cfg(test)]
    mod attempts {
        use crate::backoff::exponential;
        use chrono::Duration;

        const MULTIPLIER_TWO: f32 = 2.0;

        #[test]
        pub fn zero() {
            assert_eq!(
                exponential(&Duration::seconds(10), MULTIPLIER_TWO, 0),
                Duration::seconds(10)
            );
        }

        #[test]
        pub fn one() {
            assert_eq!(
                exponential(&Duration::seconds(10), MULTIPLIER_TWO, 1),
                Duration::seconds(20)
            );
        }

        #[test]
        pub fn two() {
            assert_eq!(
                exponential(&Duration::seconds(10), MULTIPLIER_TWO, 2),
                Duration::seconds(40)
            );
        }

        #[test]
        pub fn three() {
            assert_eq!(
                exponential(&Duration::seconds(10), MULTIPLIER_TWO, 3),
                Duration::seconds(80)
            );
        }

        #[test]
        pub fn four() {
            assert_eq!(
                exponential(&Duration::seconds(10), MULTIPLIER_TWO, 4),
                Duration::seconds(160)
            );
        }

        #[test]
        pub fn five() {
            assert_eq!(
                exponential(&Duration::seconds(10), MULTIPLIER_TWO, 5),
                Duration::seconds(320)
            );
        }
    }
}
