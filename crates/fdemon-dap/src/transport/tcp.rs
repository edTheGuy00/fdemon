//! # TCP Transport
//!
//! Re-exports the TCP server start function for use via the transport module.
//! The full TCP accept-loop implementation lives in [`crate::server`].
//!
//! This module exists to provide a symmetric API alongside [`super::stdio`],
//! so callers can use `transport::tcp::start` and `transport::stdio::run_stdio_session`
//! interchangeably.

pub use crate::server::start as start_server;
