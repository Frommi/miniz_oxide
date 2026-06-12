//! Optional cooperative cancellation for long-running operations.
//!
//! [`StopCheck`] is a single vendored type following the zero-dependency
//! pattern described in
//! <https://github.com/imazen/enough/blob/main/ZERO-DEP.md>: a clone-cheap
//! handle wrapping an optional user closure that is polled at coarse
//! intervals. [`StopCheck::none()`] costs nothing to poll, so it can be
//! threaded through internals unconditionally.

use crate::alloc::sync::Arc;
use core::fmt;
use core::sync::atomic::{AtomicBool, Ordering};

/// A clone-cheap cancellation probe polled periodically by long-running
/// operations.
///
/// The closure can capture any cancellation source — an `Arc<AtomicBool>`, a
/// channel, an async cancellation token, a deadline — so this crate does not
/// need to know about any of them.
#[derive(Clone, Default)]
pub struct StopCheck {
    inner: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
}

impl StopCheck {
    /// A check that never stops. Polling it is a single predicted branch and
    /// constructing it allocates nothing.
    // Not const: trait objects in const fn need Rust 1.61, above this crate's MSRV.
    pub fn none() -> StopCheck {
        StopCheck { inner: None }
    }

    /// Wraps a closure; the operation stops when it returns true.
    pub fn new<F>(f: F) -> StopCheck
    where
        F: Fn() -> bool + Send + Sync + 'static,
    {
        StopCheck {
            inner: Some(Arc::new(f)),
        }
    }

    /// Stops when `flag` becomes true.
    pub fn from_atomic(flag: Arc<AtomicBool>) -> StopCheck {
        StopCheck::new(move || flag.load(Ordering::Relaxed))
    }

    /// Returns true if this check could ever signal a stop.
    pub fn may_stop(&self) -> bool {
        self.inner.is_some()
    }

    /// Polls the underlying source. [`StopCheck::none()`] always returns false.
    pub fn should_stop(&self) -> bool {
        match &self.inner {
            Some(f) => f(),
            None => false,
        }
    }
}

impl fmt::Debug for StopCheck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StopCheck")
            .field("may_stop", &self.may_stop())
            .finish()
    }
}
