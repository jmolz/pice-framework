//! Provider registry lookup (pure path walking; no process spawning).
//!
//! The async `ProviderHost` lives in `pice-daemon::provider::host`. Only the
//! lookup logic lives here.

pub mod registry;
