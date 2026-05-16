//! Optional telemetry counters for GLR parser operations.

use super::GLRParser;

/// Dummy telemetry type when feature is disabled
#[cfg(not(feature = "glr_telemetry"))]
#[allow(dead_code)]
pub(super) struct TelemetryCounters;

/// Telemetry counters for GLR performance monitoring
#[cfg(feature = "glr_telemetry")]
#[derive(Debug, Default, Clone)]
pub(super) struct TelemetryCounters {
    /// Number of reduce operations performed
    pub(super) reduce_steps: usize,
    /// Number of epsilon reductions
    pub(super) epsilon_reduces: usize,
    /// Number of shift operations performed
    pub(super) shift_steps: usize,
    /// Number of times parser forked
    pub(super) fork_count: usize,
    /// Total stacks before compression
    pub(super) tops_before_compress: usize,
    /// Total stacks after compression
    pub(super) tops_after_compress: usize,
    /// Number of ambiguity packs created
    pub(super) alts_packed: usize,
    /// Maximum active stacks at any point
    pub(super) max_active_stacks: usize,
    /// Number of accept actions at EOF
    pub(super) accept_count: usize,
}

#[allow(dead_code)]
impl GLRParser {
    /// Get telemetry summary (only when telemetry feature is enabled)
    #[cfg(feature = "glr_telemetry")]
    pub fn telemetry_summary(&self) -> String {
        format!(
            "GLR Telemetry:\n  Shifts: {}\n  Reduces: {} (epsilon: {})\n  Forks: {}\n  Compression: {}/{} -> {} (packed: {})\n  Max stacks: {}\n  Accepts: {}",
            self.telemetry.shift_steps,
            self.telemetry.reduce_steps,
            self.telemetry.epsilon_reduces,
            self.telemetry.fork_count,
            self.telemetry.tops_before_compress,
            self.telemetry.tops_after_compress,
            self.telemetry.tops_after_compress,
            self.telemetry.alts_packed,
            self.telemetry.max_active_stacks,
            self.telemetry.accept_count
        )
    }

    /// Helper to update telemetry counters (no-op when feature disabled)
    #[cfg(feature = "glr_telemetry")]
    #[inline]
    pub(super) fn bump_telemetry(&mut self, f: impl FnOnce(&mut TelemetryCounters)) {
        f(&mut self.telemetry);
    }

    #[cfg(not(feature = "glr_telemetry"))]
    #[inline]
    #[allow(dead_code)]
    pub(super) fn bump_telemetry(&mut self, _f: impl FnOnce(&mut TelemetryCounters)) {
        // No-op when telemetry is disabled
    }
}
