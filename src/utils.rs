pub mod tui {
    use std::time::{Duration, SystemTime};

    pub struct FPSLimiter {
        refresh_rate: Duration,
        allowed_at: Option<SystemTime>,
    }

    impl FPSLimiter {
        pub fn default(max_fps: u32) -> Self {
            let refresh_rate = Duration::from_micros(1_000_000 / (max_fps as u64));
            return FPSLimiter {
                refresh_rate,
                allowed_at: None,
            };
        }

        pub fn check_allowed(&mut self, now: SystemTime) -> bool {
            match self.allowed_at {
                Some(allowed_at) => {
                    if allowed_at >= now {
                        return false;
                    } else {
                        self.allowed_at = Some(now + self.refresh_rate);
                        return true;
                    }
                }
                None => {
                    self.allowed_at = Some(now + self.refresh_rate);
                    true
                }
            }
        }
    }
    #[cfg(test)]
    mod tests {
        use std::time::{Duration, UNIX_EPOCH};

        use crate::utils::tui::FPSLimiter;

        #[test]
        fn test_allowed() {
            let mut fps_limiter = FPSLimiter::default(100);
            let time = UNIX_EPOCH + Duration::from_secs(10);
            // first call is allowed
            assert!(fps_limiter.check_allowed(time));
            // next calls are not allowed until time is passed
            let next_call_at = time + Duration::from_millis(5);
            assert!(!fps_limiter.check_allowed(next_call_at));
            // next call is allowed when time is passed
            assert!(fps_limiter.check_allowed(time + Duration::from_millis(11)));
        }
    }
}

pub mod mavlink {
    pub fn decode_param_id(param_id: &[u8; 16]) -> String {
        param_id
            .iter()
            .filter(|&b| *b != 0)
            .map(|&b| char::from(b))
            .collect()
    }

    #[cfg(test)]
    mod tests {
        use crate::utils::mavlink::decode_param_id;

        #[test]
        fn test_decode_param_id() {
            let mut array = [0u8; 16];
            for (i, ch) in "TEST_PARAM".chars().enumerate() {
                array[i] = ch as u8;
            }
            assert_eq!(decode_param_id(&array), "TEST_PARAM".to_string());
        }
    }
}
