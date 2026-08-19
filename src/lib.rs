pub mod signing;

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
