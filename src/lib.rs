pub mod core;
pub mod ffi;
pub mod showcase;

// Re-export core functionality for convenience
pub use core::{Deloxide, DeadlockInfo};
pub use showcase::showcase;