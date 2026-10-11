//! Core problem definition, parser, and transport-neutral request handler.

pub mod algorithms;
pub mod api;
pub mod computation;
pub mod model;
pub mod parser;

pub use api::{Request, Response, execute, execute_json};
pub use parser::parse_problem;
