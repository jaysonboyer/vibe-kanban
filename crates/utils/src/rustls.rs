//! Shared rustls crypto provider installer.
//!
//! `rustls::crypto::aws_lc_rs::default_provider().install_default()` writes
//! process-global state. A second call returns `Err(_)`; today's call sites
//! `.expect(...)` it and would panic when both the main server and the MCP
//! binary run in the same process (e.g., Tauri). This helper uses
//! `std::sync::Once` so subsequent calls are a no-op + warn instead.
//! VKSTART-09 / CONCERNS.md "Rustls Default Provider Installation Happens Twice".

use std::sync::Once;

static INIT: Once = Once::new();

pub fn install_default_provider() {
    INIT.call_once(|| {
        if rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .is_err()
        {
            tracing::warn!(
                "rustls default provider was already installed by another component; continuing"
            );
        }
    });
}
