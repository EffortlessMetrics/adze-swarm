//! SRP helpers for rayon initialization during concurrency-cap bootstrap.

use super::super::rayon::init_rayon_global_once;

/// Initialize the global rayon pool or panic with a consistent message.
pub(super) fn init_or_panic(rayon_threads: usize) {
    if let Err(message) = init_rayon_global_once(rayon_threads) {
        panic!("failed to initialize rayon global thread pool: {message}");
    }
}
