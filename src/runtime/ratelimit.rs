// ─── Rate limiter for gateways (Phase 7 Item 22) ──────────────────────────────

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

#[derive(Debug)]
pub struct RateLimiter {
    window_secs: u64,
    max_requests: u64,
    burst: u64,
    buckets: Mutex<HashMap<String, Bucket>>,
}

#[derive(Debug, Clone)]
struct Bucket {
    tokens: u64,
    last_refill: Instant,
}

impl RateLimiter {
    pub fn new(max_requests_per_minute: u64, burst: u64) -> Self {
        Self {
            window_secs: 60,
            max_requests: max_requests_per_minute,
            burst: burst.max(1),
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Check if a user_id is allowed to make a request.
    /// Returns (allowed, remaining, reset_seconds).
    pub fn check(&self, user_id: &str) -> (bool, u64, u64) {
        let mut buckets = self.buckets.lock().unwrap();
        let now = Instant::now();
        let bucket = buckets.entry(user_id.to_string()).or_insert(Bucket {
            tokens: self.burst,
            last_refill: now,
        });

        // Refill tokens based on elapsed time
        let elapsed = now.duration_since(bucket.last_refill).as_secs();
        if elapsed > 0 {
            let refill =
                (elapsed as f64 / self.window_secs as f64 * self.max_requests as f64) as u64;
            bucket.tokens = (bucket.tokens + refill).min(self.burst);
            bucket.last_refill = now;
        }

        if bucket.tokens > 0 {
            bucket.tokens -= 1;
            (true, bucket.tokens, self.window_secs)
        } else {
            (false, 0, self.window_secs)
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(60, 10)
    }
}

// ─── TUI Interrupt Handler (Phase 6 Item 17) ──────────────────────────────────

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct InterruptHandler {
    interrupted: Arc<AtomicBool>,
}

impl InterruptHandler {
    pub fn new() -> Self {
        let handler = Self {
            interrupted: Arc::new(AtomicBool::new(false)),
        };
        let flag = handler.interrupted.clone();

        // Set up Ctrl+C handler
        ctrlc::set_handler(move || {
            flag.store(true, Ordering::SeqCst);
        })
        .ok();

        handler
    }

    pub fn is_interrupted(&self) -> bool {
        self.interrupted.load(Ordering::SeqCst)
    }

    pub fn reset(&self) {
        self.interrupted.store(false, Ordering::SeqCst);
    }
}

impl Default for InterruptHandler {
    fn default() -> Self {
        Self::new()
    }
}
