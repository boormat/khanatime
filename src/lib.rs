pub mod signing;

// One `run_in_browser` declaration per test binary; the main.rs copy covers the
// bin target's tests (see the note there).
#[cfg(all(test, target_arch = "wasm32"))]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

// Vec of str to strings.
// let a= stringify(["a", "b", "c"]);
pub fn stringify<const N: usize>(a: [&str; N]) -> [String; N] {
    a.map(String::from)
}

/// Minimal `log!` macro (console.log for wasm).
/// Use `log!("fmt {}", x)`.
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        $crate::web_log(&format!($($arg)*))
    };
}

pub fn web_log(msg: &str) {
    web_sys::console::log_1(&js_sys::JsString::from(msg));
}
