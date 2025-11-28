// Feature-gated storage implementations

#[cfg(feature = "file")]
pub mod file;

#[cfg(feature = "wasm")]
pub mod wasm;

// Re-export the appropriate storage module as `full` based on enabled features
#[cfg(feature = "file")]
pub use file as full;

#[cfg(feature = "wasm")]
pub use wasm as full;

// Trait for storage abstraction
pub trait Storage {
    fn new() -> Self;
}
