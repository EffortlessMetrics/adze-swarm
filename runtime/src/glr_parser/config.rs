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
