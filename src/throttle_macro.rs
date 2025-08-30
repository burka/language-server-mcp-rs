//! Throttling has been moved to LSP client send_message method
//! 
//! This provides the ultimate single injection point - every LSP request
//! is automatically throttled at the protocol level.

