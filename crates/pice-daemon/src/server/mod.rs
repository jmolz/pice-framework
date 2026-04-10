//! Daemon RPC server — socket listener, transport impls, auth, router.
//!
//! Populated across T15–T18:
//! - T15: Unix socket transport (`server::unix`)
//! - T16: Windows named pipe transport (`server::windows`)
//! - T17: authentication token generation + validation (`server::auth`)
//! - T18: RPC method dispatch table (`server::router`)
