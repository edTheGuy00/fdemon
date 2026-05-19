//! Abstraction trait over [`VmRequestHandle`] for testability.
//!
//! [`VmRequestApi`] defines the minimal async interface needed by polling tasks
//! (currently only `spawn_timeline_polling`). Production code passes the
//! concrete [`VmRequestHandle`]; tests substitute a [`MockVmRequestApi`].
//!
//! ## Design notes
//!
//! - The trait uses `async fn` directly (stabilised in Rust 1.75+, MSRV here
//!   is 1.77.2), so no `async-trait` proc-macro or `trait-variant` is needed.
//! - Only the two methods actually called by polling code are included:
//!   `request` and `call_extension`. Higher-level helpers like
//!   `main_isolate_id` are intentionally omitted — callers that need them
//!   invoke free functions (e.g. `get_vm_timeline_micros`) which are also
//!   generic over `VmRequestApi`.
//! - The trait is unconditionally `pub` so that `fdemon-app`'s production
//!   `spawn_timeline_polling` signature can name it without a feature gate.
//!   Test-only infrastructure (mocks) remains gated by `#[cfg(test)]`.

use std::collections::HashMap;

use fdemon_core::prelude::*;

/// Minimal async interface over a VM Service request channel.
///
/// Implemented by [`super::client::VmRequestHandle`] for production use and by
/// test doubles for unit/integration testing of polling tasks.
///
/// Both methods mirror the signatures on `VmRequestHandle` exactly so that
/// `impl VmRequestApi` and the concrete type are interchangeable at every call
/// site.
pub trait VmRequestApi {
    /// Send a JSON-RPC request and await the response.
    ///
    /// Returns the `result` field of a successful response, or an error for
    /// JSON-RPC errors and transport failures.
    fn request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> impl std::future::Future<Output = Result<serde_json::Value>> + Send;

    /// Call a Flutter service extension method on the given isolate.
    ///
    /// Automatically prepends `isolateId` to the params map.
    fn call_extension(
        &self,
        method: &str,
        isolate_id: &str,
        args: Option<HashMap<String, String>>,
    ) -> impl std::future::Future<Output = Result<serde_json::Value>> + Send;
}
