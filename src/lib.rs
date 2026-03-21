//! Core library entry point for FluidNC communication.
//!
//! Re-exports primary traits, types, and driver implementation.
pub mod types;
pub mod traits;
pub mod transport;
pub mod driver;

#[cfg(test)]
mod tests;

pub use types::*;
pub use traits::*;
pub use driver::*;
