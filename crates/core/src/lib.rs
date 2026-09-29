//! Core problem definition, parser, and transport-neutral request handler.

pub mod labels;
pub mod parser;
pub mod problem;
pub mod protocol;

pub use parser::parse_problem;
pub use protocol::{Request, Response, execute, execute_json};
