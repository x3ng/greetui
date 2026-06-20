// This file is kept for backward compatibility.
// The actual types and logic have been moved to:
// - config.rs: CLI argument parsing (Config)
// - state.rs: Application state (App, AuthState, etc.)
//
// Re-export for existing imports.
pub use crate::state::GreetAlign;
