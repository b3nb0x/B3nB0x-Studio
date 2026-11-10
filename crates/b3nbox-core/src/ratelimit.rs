use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_rpm")]
    pub requests_per_minute: u32,
    #[serde(default = "default_llm_rpm")]
    pub llm_requests_per_minute: u32,
}

fn default_enabled() -> bool {
    true
}

fn default_rpm() -> u32 {
    150
}

fn default_llm_rpm() -> u32 {
    30
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            requests_per_minute: default_rpm(),
            llm_requests_per_minute: default_llm_rpm(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitStatus {
    pub enabled: bool,
    pub requests_per_minute: u32,
    pub llm_requests_per_minute: u32,
}

struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

impl Bucket {
    fn new(capacity: f64) -> Self {
        Self {
            tokens: capacity,
            last_refill: Instant::now(),
        }
    }

    /// Fuellt den Bucket nach und zieht ggf. ein Token ab.
    /// Gibt false zurueck, wenn kein Token verfuegbar war.
    fn take(&mut self, capacity: f64) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * (capacity / 60.0)).min(capacity);
        self.last_refill = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Sekunden bis zum naechsten verfuegbaren Token.
    fn retry_after_secs(&self, capacity: f64) -> u64 {
        let deficit = 1.0 - self.tokens;
        (deficit / (capacity / 60.0)).ceil() as u64
    }
}

pub struct RateLimiter {
    config: Mutex<RateLimitConfig>,
    buckets: Mutex<HashMap<String, Bucket>>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config: Mutex::new(config),
            buckets: Mutex::new(HashMap::new()),
        }
    }

    pub fn status(&self) -> RateLimitStatus {
        let config = self.config.lock().unwrap();
        RateLimitStatus {
            enabled: config.enabled,
            requests_per_minute: config.requests_per_minute,
            llm_requests_per_minute: config.llm_requests_per_minute,
        }
    }

    /// Limits zur Laufzeit anpassen (Admin).
    pub fn update(&self, requests_per_minute: u32, llm_requests_per_minute: u32) -> RateLimitStatus {
        {
            let mut config = self.config.lock().unwrap();
            config.requests_per_minute = requests_per_minute.max(1);
            config.llm_requests_per_minute = llm_requests_per_minute.max(1);
        }
        self.buckets.lock().unwrap().clear();
        self.status()
    }

    /// Prueft einen normalen Request. Ok(()) oder Err(retry_after_secs).
    pub fn check(&self, client: &str) -> Result<(), u64> {
        let (enabled, capacity) = {
            let config = self.config.lock().unwrap();
            (config.enabled, config.requests_per_minute)
        };
        if !enabled {
            return Ok(());
        }
        let mut buckets = self.buckets.lock().unwrap();
        let bucket = buckets.entry(client.to_string()).or_insert_with(|| Bucket::new(capacity as f64));
        if bucket.take(capacity as f64) {
            Ok(())
        } else {
            Err(bucket.retry_after_secs(capacity as f64))
        }
    }

    /// Prueft einen teueren LLM-Request (eigenes, niedrigeres Limit).
    pub fn check_llm(&self, client: &str) -> Result<(), u64> {
        let (enabled, capacity) = {
            let config = self.config.lock().unwrap();
            (config.enabled, config.llm_requests_per_minute)
        };
        if !enabled {
            return Ok(());
        }
        let mut buckets = self.buckets.lock().unwrap();
        let key = format!("{client}:llm");
        let bucket = buckets.entry(key).or_insert_with(|| Bucket::new(capacity as f64));
        if bucket.take(capacity as f64) {
            Ok(())
        } else {
            Err(bucket.retry_after_secs(capacity as f64))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_burst_up_to_limit() {
        let limiter = RateLimiter::new(RateLimitConfig {
            requests_per_minute: 5,
            llm_requests_per_minute: 2,
            ..Default::default()
        });
        for _ in 0..5 {
            assert!(limiter.check("client-a").is_ok());
        }
        assert!(limiter.check("client-a").is_err(), "6. Request muss abgelehnt werden");
    }

    #[test]
    fn clients_are_isolated() {
        let limiter = RateLimiter::new(RateLimitConfig {
            requests_per_minute: 2,
            llm_requests_per_minute: 2,
            ..Default::default()
        });
        assert!(limiter.check("a").is_ok());
        assert!(limiter.check("a").is_ok());
        assert!(limiter.check("a").is_err());
        assert!(limiter.check("b").is_ok(), "Anderer Client hat eigenes Limit");
    }

    #[test]
    fn llm_limit_independent() {
        let limiter = RateLimiter::new(RateLimitConfig {
            requests_per_minute: 100,
            llm_requests_per_minute: 2,
            ..Default::default()
        });
        assert!(limiter.check_llm("a").is_ok());
        assert!(limiter.check_llm("a").is_ok());
        assert!(limiter.check_llm("a").is_err(), "LLM-Limit greift frueher");
        assert!(limiter.check("a").is_ok(), "Normales Limit unberuehrt");
    }

    #[test]
    fn retry_after_is_positive() {
        let limiter = RateLimiter::new(RateLimitConfig {
            requests_per_minute: 1,
            llm_requests_per_minute: 1,
            ..Default::default()
        });
        assert!(limiter.check("a").is_ok());
        match limiter.check("a") {
            Err(retry) => assert!((1..=60).contains(&retry)),
            Ok(()) => panic!("sollte abgelehnt werden"),
        }
    }

    #[test]
    fn runtime_update_changes_limits() {
        let limiter = RateLimiter::new(RateLimitConfig::default());
        assert_eq!(limiter.status().requests_per_minute, 150);
        limiter.update(10, 5);
        assert_eq!(limiter.status().requests_per_minute, 10);
        assert_eq!(limiter.status().llm_requests_per_minute, 5);
    }

    #[test]
    fn disabled_limiter_allows_all() {
        let limiter = RateLimiter::new(RateLimitConfig {
            enabled: false,
            requests_per_minute: 1,
            llm_requests_per_minute: 1,
        });
        for _ in 0..100 {
            assert!(limiter.check("a").is_ok());
            assert!(limiter.check_llm("a").is_ok());
        }
    }
}
