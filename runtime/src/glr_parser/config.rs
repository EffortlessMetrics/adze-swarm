//! Configuration helpers for the GLR parser.

/// Default threshold for pointer-based dedup.
pub const DEFAULT_SAFE_DEDUP_THRESHOLD: usize = 10;

/// Returns the stack-count threshold at which pointer-based deduplication is enabled.
#[inline]
pub fn safe_dedup_threshold() -> usize {
    if let Some(s) = option_env!("ADZE_SAFE_DEDUP_N")
        && let Ok(n) = s.parse::<usize>()
    {
        return n;
    }
    std::env::var("ADZE_SAFE_DEDUP_N")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_SAFE_DEDUP_THRESHOLD)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialize env-var access so concurrent tests don't race.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env_var<R>(value: Option<&str>, body: impl FnOnce() -> R) -> R {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var("ADZE_SAFE_DEDUP_N").ok();
        // SAFETY: env mutation guarded by the test-local ENV_LOCK to keep
        // concurrent tests from racing on the same variable.
        unsafe {
            match value {
                Some(v) => std::env::set_var("ADZE_SAFE_DEDUP_N", v),
                None => std::env::remove_var("ADZE_SAFE_DEDUP_N"),
            }
        }
        let result = body();
        // SAFETY: same as above; restore prior value for hermeticity.
        unsafe {
            match previous {
                Some(v) => std::env::set_var("ADZE_SAFE_DEDUP_N", v),
                None => std::env::remove_var("ADZE_SAFE_DEDUP_N"),
            }
        }
        result
    }

    #[test]
    fn default_threshold_is_ten() {
        assert_eq!(DEFAULT_SAFE_DEDUP_THRESHOLD, 10);
    }

    #[test]
    fn safe_dedup_threshold_uses_default_when_env_unset() {
        // option_env! is resolved at compile time and may be unset in the
        // testing environment; with the runtime var unset we fall back to
        // the documented default.
        let observed = with_env_var(None, safe_dedup_threshold);
        if option_env!("ADZE_SAFE_DEDUP_N").is_none() {
            assert_eq!(observed, DEFAULT_SAFE_DEDUP_THRESHOLD);
        }
    }

    #[test]
    fn safe_dedup_threshold_reads_runtime_env_var() {
        if option_env!("ADZE_SAFE_DEDUP_N").is_some() {
            // Compile-time override wins and we cannot exercise the runtime
            // path. Skip rather than fail.
            return;
        }
        let observed = with_env_var(Some("42"), safe_dedup_threshold);
        assert_eq!(observed, 42);
    }

    #[test]
    fn safe_dedup_threshold_ignores_unparseable_env() {
        if option_env!("ADZE_SAFE_DEDUP_N").is_some() {
            return;
        }
        let observed = with_env_var(Some("not-a-number"), safe_dedup_threshold);
        assert_eq!(observed, DEFAULT_SAFE_DEDUP_THRESHOLD);
    }

    #[test]
    fn safe_dedup_threshold_accepts_zero() {
        if option_env!("ADZE_SAFE_DEDUP_N").is_some() {
            return;
        }
        let observed = with_env_var(Some("0"), safe_dedup_threshold);
        assert_eq!(observed, 0);
    }
}
