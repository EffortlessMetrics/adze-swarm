//! Single-responsibility bootstrap for concurrency caps initialization.

use crate::concurrency_caps::env::current_caps;

mod logging;
pub mod policy;
mod rayon_init;

pub use crate::concurrency_caps::env::{
    ConcurrencyCaps, DEFAULT_RAYON_NUM_THREADS, DEFAULT_TOKIO_WORKER_THREADS,
    RAYON_NUM_THREADS_ENV, TOKIO_WORKER_THREADS_ENV,
};
pub use policy::bootstrap_caps;

/// Initialize concurrency caps by reading the current process environment.
pub fn init_concurrency_caps() {
    init_concurrency_caps_with_caps(current_caps());
}

/// Initialize concurrency caps using an explicit configuration.
pub fn init_concurrency_caps_with_caps(caps: ConcurrencyCaps) {
    let caps = bootstrap_caps(caps);

    rayon_init::init_or_panic(caps.rayon_threads);
    logging::emit_caps_initialized(caps);
}

#[cfg(test)]
mod tests {
    use super::{bootstrap_caps, init_concurrency_caps_with_caps};
    use crate::concurrency_caps::env::{
        ConcurrencyCaps, DEFAULT_RAYON_NUM_THREADS, DEFAULT_TOKIO_WORKER_THREADS,
        RAYON_NUM_THREADS_ENV, TOKIO_WORKER_THREADS_ENV,
    };

    #[test]
    fn init_with_zero_rayon_threads_normalizes_to_minimum() {
        let caps = bootstrap_caps(ConcurrencyCaps {
            rayon_threads: 0,
            tokio_worker_threads: 2,
        });
        assert_eq!(caps.rayon_threads, 1);
    }

    #[test]
    fn init_with_zero_is_idempotent() {
        init_concurrency_caps_with_caps(ConcurrencyCaps {
            rayon_threads: 0,
            tokio_worker_threads: 2,
        });
        init_concurrency_caps_with_caps(ConcurrencyCaps {
            rayon_threads: 0,
            tokio_worker_threads: 2,
        });
    }

    #[test]
    fn init_with_positive_rayon_threads_succeeds() {
        init_concurrency_caps_with_caps(ConcurrencyCaps {
            rayon_threads: 4,
            tokio_worker_threads: 2,
        });
    }

    #[test]
    fn bootstrap_preserves_nonzero_rayon_threads() {
        let caps = bootstrap_caps(ConcurrencyCaps {
            rayon_threads: 8,
            tokio_worker_threads: 3,
        });
        assert_eq!(caps.rayon_threads, 8);
    }

    #[test]
    fn reexported_defaults_are_accessible() {
        assert_eq!(DEFAULT_RAYON_NUM_THREADS, 4);
        assert_eq!(DEFAULT_TOKIO_WORKER_THREADS, 2);
    }

    #[test]
    fn reexported_env_var_names_are_correct() {
        assert_eq!(RAYON_NUM_THREADS_ENV, "RAYON_NUM_THREADS");
        assert_eq!(TOKIO_WORKER_THREADS_ENV, "TOKIO_WORKER_THREADS");
    }
}
