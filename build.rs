//! Bake `KHANATIME_APP_VERSION` for the wasm/binary.
//!
//! - If `KHANATIME_APP_VERSION` is already set (CI release/preview), use it.
//! - Else if `KHANATIME_DEV_SHA=1` (or preview builds), use `dev-<shortsha>`.
//! - Else use `CARGO_PKG_VERSION` (local `cargo build` / tagged release default).

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=KHANATIME_APP_VERSION");
    println!("cargo:rerun-if-env-changed=KHANATIME_DEV_SHA");

    if let Ok(v) = std::env::var("KHANATIME_APP_VERSION") {
        if !v.is_empty() {
            println!("cargo:rustc-env=KHANATIME_APP_VERSION={v}");
            return;
        }
    }

    let pkg = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".into());
    let want_dev = std::env::var("KHANATIME_DEV_SHA").ok().as_deref() == Some("1");
    if want_dev {
        let sha = git_short_sha().unwrap_or_else(|| "unknown".into());
        println!("cargo:rustc-env=KHANATIME_APP_VERSION=dev-{sha}");
    } else {
        println!("cargo:rustc-env=KHANATIME_APP_VERSION={pkg}");
    }
}

fn git_short_sha() -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
