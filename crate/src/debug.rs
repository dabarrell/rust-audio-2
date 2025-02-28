use std::sync::atomic::{AtomicBool, Ordering};

// Debug flag - can be enabled via environment variable
static DEBUG: AtomicBool = AtomicBool::new(false);

/// Enable or disable debug output
pub fn set_debug(enabled: bool) {
    DEBUG.store(enabled, Ordering::Relaxed);
}

/// Get the current debug state
pub fn is_debug_enabled() -> bool {
    DEBUG.load(Ordering::Relaxed)
}

// Debug macro for conditional printing
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {{
        // if $crate::debug::is_debug_enabled() {
            #[cfg(target_arch = "wasm32")]
            {
                use wasm_bindgen::JsValue;
                web_sys::console::log_1(&format!($($arg)*).into());
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                println!($($arg)*);
            }
        // }
    }};
}
