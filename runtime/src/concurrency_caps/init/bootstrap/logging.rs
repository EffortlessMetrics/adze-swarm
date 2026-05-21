//! SRP helpers for bootstrap initialization logging.

use super::{ConcurrencyCaps, RAYON_NUM_THREADS_ENV, TOKIO_WORKER_THREADS_ENV};

/// Emit the canonical concurrency-cap initialization line.
pub(super) fn emit_caps_initialized(caps: ConcurrencyCaps) {
    eprintln!(
        "Concurrency caps initialized: {RAYON_NUM_THREADS_ENV}={}, {TOKIO_WORKER_THREADS_ENV}={}",
        caps.rayon_threads, caps.tokio_worker_threads
    );
}
