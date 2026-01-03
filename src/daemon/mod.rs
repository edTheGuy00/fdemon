//! Flutter daemon infrastructure layer

pub mod process;
pub mod protocol;

pub use process::FlutterProcess;
pub use protocol::{strip_brackets, RawMessage};
