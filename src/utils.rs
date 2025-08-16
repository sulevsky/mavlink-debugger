use std::time::SystemTime;

pub struct FPSLimiter {
    refresh_rate_millis: u128,
    allowed_at: Option<u128>,
}

impl FPSLimiter {
    pub fn default(max_fps: i32) -> Self {
        let refresh_rate_millis = 1000u128 / (max_fps as u128);
        return FPSLimiter {
            refresh_rate_millis,
            allowed_at: None,
        };
    }
    pub fn check_allowed(&mut self) -> bool {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        match self.allowed_at {
            Some(allowed_at) => {
                if allowed_at > now {
                    return false;
                } else {
                    self.allowed_at = Some(now + self.refresh_rate_millis);
                    return true;
                }
            }
            None => {
                self.allowed_at = Some(now);
                true
            }
        }
    }
}
