//! Camera QR scanning for parcel import (wasm only).
//!
//! Uses the browser's `BarcodeDetector` (Chrome/Edge/Android) over a live
//! `getUserMedia` video feed.  Decoded `khanatime_qr:` / `khanatime_parcel:`
//! strings are accumulated; once a chunked parcel's frames are all present the
//! parcel is re-joined and imported through the same path as pasting
//! (`sync::import_parcel_text`).  Feature-detects `BarcodeDetector` and falls
//! back to a message (paste instead) where it's missing (e.g. Firefox).

use crate::Model;
use std::cell::RefCell;
use wasm_bindgen::{JsCast, JsValue};

/// Matches the `<video id="kt-scan-video">` element rendered by `view_handoff`.
const VIDEO_ID: &str = "kt-scan-video";
/// How often the camera frame is handed to the detector, in ms.
const DETECT_INTERVAL_MS: i32 = 220;

struct ScanSession {
    stream: web_sys::MediaStream,
    interval: i32,
    seen: std::collections::HashSet<(String, u32)>,
    frames: Vec<crate::services::qr::Frame>,
}

thread_local! {
    static SCAN: RefCell<Option<ScanSession>> = const { RefCell::new(None) };
}

/// Ask for the camera and begin scanning.  No-op if a session is already live
/// or the browser can't do it.
pub fn start_scan(model: Model) {
    stop_scan();
    model.app.scan_status.set("Starting camera…".to_string());
    wasm_bindgen_futures::spawn_local(async move {
        run_scan(model).await;
    });
}

async fn run_scan(model: Model) {
    let Some(window) = web_sys::window() else {
        return;
    };
    // Feature-detect BarcodeDetector.
    let ctor = match js_sys::Reflect::get(&window, &"BarcodeDetector".into()) {
        Ok(v) if !v.is_undefined() && !v.is_null() => v,
        _ => {
            model
                .app
                .scan_status
                .set("QR scanning needs Chrome/Edge — paste the parcel instead.".to_string());
            return;
        }
    };
    let opts = js_sys::Object::new();
    let formats = js_sys::Array::of1(&"qr_code".into());
    let _ = js_sys::Reflect::set(&opts, &"formats".into(), &formats);
    let ctor_fn: js_sys::Function = ctor.unchecked_into();
    let detector = match js_sys::Reflect::construct(&ctor_fn, &js_sys::Array::of1(&opts)) {
        Ok(d) => d,
        Err(_) => {
            model
                .app
                .scan_status
                .set("QR scanning unavailable here.".to_string());
            return;
        }
    };
    let detect_fn: js_sys::Function = js_sys::Reflect::get(&detector, &"detect".into())
        .unwrap()
        .unchecked_into();

    let nav = window.navigator();
    let Ok(media) = nav.media_devices() else {
        model
            .app
            .scan_status
            .set("No camera available.".to_string());
        return;
    };
    let constraints = js_sys::Object::new();
    let video_constraint = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &video_constraint,
        &"facingMode".into(),
        &"environment".into(),
    );
    let _ = js_sys::Reflect::set(&constraints, &"video".into(), &video_constraint);
    let constraints: web_sys::MediaStreamConstraints = constraints.unchecked_into();
    let stream = match media.get_user_media_with_constraints(&constraints) {
        Ok(p) => match wasm_bindgen_futures::JsFuture::from(p).await {
            Ok(s) => s.unchecked_into::<web_sys::MediaStream>(),
            Err(_) => {
                model
                    .app
                    .scan_status
                    .set("Camera permission denied.".to_string());
                return;
            }
        },
        Err(_) => {
            model
                .app
                .scan_status
                .set("Camera permission denied.".to_string());
            return;
        }
    };

    let Some(video) = window
        .document()
        .and_then(|d| d.get_element_by_id(VIDEO_ID))
        .map(|el| el.unchecked_into::<web_sys::HtmlVideoElement>())
    else {
        model
            .app
            .scan_status
            .set("Scanner view missing — reload the page.".to_string());
        return;
    };
    video.set_src_object(Some(&stream));
    let _ = video.play();

    let interval = spawn_detect_loop(model, detector, detect_fn, video);
    SCAN.with(|s| {
        *s.borrow_mut() = Some(ScanSession {
            stream,
            interval,
            seen: Default::default(),
            frames: Vec::new(),
        })
    });
    model.app.scan_active.set(true);
    model
        .app
        .scan_status
        .set("Point the other phone's camera at the QR.".to_string());
}

/// Every tick, run `detect` on the current video frame and feed each decoded
/// string to the accumulator.  Returns the interval handle.
fn spawn_detect_loop(
    model: Model,
    detector: JsValue,
    detect_fn: js_sys::Function,
    video: web_sys::HtmlVideoElement,
) -> i32 {
    let window = web_sys::window().expect("window");
    let closure = wasm_bindgen::closure::Closure::<dyn FnMut()>::wrap(Box::new(move || {
        let detector = detector.clone();
        let detect_fn = detect_fn.clone();
        let video = video.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let args = js_sys::Array::of1(&video);
            let Ok(promise) = detect_fn.apply(&detector, &args) else {
                return;
            };
            let promise: js_sys::Promise = promise.unchecked_into();
            let Ok(value) = wasm_bindgen_futures::JsFuture::from(promise).await else {
                return;
            };
            let codes = value.unchecked_into::<js_sys::Array>();
            for i in 0..codes.length() {
                let raw = js_sys::Reflect::get(&codes.get(i), &"rawValue".into())
                    .ok()
                    .and_then(|v| v.as_string());
                if let Some(text) = raw {
                    handle_scan_string(model, &text);
                }
            }
        });
    }));
    let id = window
        .set_interval_with_callback_and_timeout_and_arguments_0(
            closure.as_ref().unchecked_ref(),
            DETECT_INTERVAL_MS,
        )
        .unwrap_or_default();
    closure.forget();
    id
}

/// Accumulate a decoded string: a chunked frame joins the session (and triggers
/// import once complete); a whole parcel imports directly.
fn handle_scan_string(model: Model, text: &str) {
    if let Ok(frame) = crate::services::qr::unpack_frame(text) {
        let complete = SCAN.with(|sc| {
            let mut b = sc.borrow_mut();
            let Some(sess) = b.as_mut() else {
                return false;
            };
            if sess.seen.insert((frame.id.clone(), frame.index)) {
                sess.frames.push(frame.clone());
                crate::services::qr::assemble_frames(&sess.frames).is_ok()
            } else {
                false
            }
        });
        if complete {
            let parcel = SCAN.with(|sc| {
                sc.borrow()
                    .as_ref()
                    .and_then(|s| crate::services::qr::assemble_frames(&s.frames).ok())
            });
            if let Some(parcel) = parcel {
                finish_scan(model, &parcel);
            }
        }
        return;
    }
    if crate::services::qr::unpack_parcel(text).is_ok() {
        finish_scan(model, text);
    }
}

fn finish_scan(model: Model, parcel: &str) {
    model.app.scan_active.set(false);
    stop_scan();
    crate::sync::import_parcel_text(model, parcel);
}

/// Tear down the scan session: clear the timer, stop tracks, clear the video.
pub fn stop_scan() {
    let (interval, stream) = SCAN.with(|sc| {
        let b = sc.borrow();
        match b.as_ref() {
            Some(s) => (Some(s.interval), Some(s.stream.clone())),
            None => (None, None),
        }
    });
    if let Some(interval) = interval {
        if let Some(window) = web_sys::window() {
            window.clear_interval_with_handle(interval);
        }
    }
    if let Some(stream) = stream {
        for track in stream.get_tracks().iter() {
            let track: web_sys::MediaStreamTrack = track.unchecked_into();
            track.stop();
        }
    }
    SCAN.with(|sc| *sc.borrow_mut() = None);
    if let Some(window) = web_sys::window() {
        if let Some(el) = window
            .document()
            .and_then(|d| d.get_element_by_id(VIDEO_ID))
        {
            let video: web_sys::HtmlVideoElement = el.unchecked_into();
            video.set_src_object(None);
        }
    }
}
