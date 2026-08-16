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
/// or the browser can't do it.  The viewfinder is shown immediately (a hidden
/// video doesn't produce frames for the detector), then hidden again on error.
pub fn start_scan(model: Model) {
    stop_scan();
    model.app.scan_active.set(true);
    model.app.scan_status.set("Starting camera…".to_string());
    wasm_bindgen_futures::spawn_local(async move {
        run_scan(model).await;
    });
}

async fn run_scan(model: Model) {
    let Some(window) = web_sys::window() else {
        model.app.scan_active.set(false);
        return;
    };
    // Feature-detect BarcodeDetector.
    let ctor = match js_sys::Reflect::get(&window, &"BarcodeDetector".into()) {
        Ok(v) if !v.is_undefined() && !v.is_null() => v,
        _ => {
            model.app.scan_active.set(false);
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
            model.app.scan_active.set(false);
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
        model.app.scan_active.set(false);
        model
            .app
            .scan_status
            .set("No camera available.".to_string());
        return;
    };
    // Prefer the rear camera via a nested `{ facingMode: "environment" }`
    // constraint (ideal, so a front-only device still works).
    let video_constraint = js_sys::Object::new();
    let facing = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&facing, &"ideal".into(), &"environment".into());
    let _ = js_sys::Reflect::set(&video_constraint, &"facingMode".into(), &facing);
    let constraints = web_sys::MediaStreamConstraints::new();
    constraints.set_video(&video_constraint.into());
    let stream = match media.get_user_media_with_constraints(&constraints) {
        Ok(p) => match wasm_bindgen_futures::JsFuture::from(p).await {
            Ok(s) => s.unchecked_into::<web_sys::MediaStream>(),
            Err(_) => {
                model.app.scan_active.set(false);
                model
                    .app
                    .scan_status
                    .set("Camera permission denied.".to_string());
                return;
            }
        },
        Err(_) => {
            model.app.scan_active.set(false);
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
        model.app.scan_active.set(false);
        model
            .app
            .scan_status
            .set("Scanner view missing — reload the page.".to_string());
        return;
    };
    // Set these via JS (not just view attributes) so autoplay can't be blocked.
    video.set_muted(true);
    video.set_autoplay(true);
    video.set_src_object(Some(&stream));
    let n = video_track_count(&stream);
    if let Err(e) = video.play() {
        model
            .app
            .scan_status
            .set(format!("Camera start failed: {e:?} — check permission."));
    } else if n == 0 {
        model
            .app
            .scan_status
            .set("Camera has no video track — try the other camera.".to_string());
    } else {
        model.app.scan_status.set(format!(
            "Camera on ({n} track{s}) — point at the QR.",
            s = if n == 1 { "" } else { "s" }
        ));
    }

    let interval = spawn_detect_loop(model, detector, detect_fn, stream.clone());
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
        .set("Camera on — point at the QR.".to_string());
}

fn video_track_count(stream: &web_sys::MediaStream) -> usize {
    stream.get_video_tracks().length() as usize
}

/// Every tick, run `detect` on the current video frame and feed each decoded
/// string to the accumulator.  Returns the interval handle.
///
/// The video element is re-queried each tick and the stream re-bound if it was
/// recreated: Sycamore rebuilds the view when its signals change (e.g. the
/// status line), so a video element we bound once can be replaced by a fresh,
/// stream-less one — re-binding keeps the on-screen element live.
fn spawn_detect_loop(
    model: Model,
    detector: JsValue,
    detect_fn: js_sys::Function,
    stream: web_sys::MediaStream,
) -> i32 {
    let window = web_sys::window().expect("window");
    let tick = std::cell::Cell::new(0u32);
    let window_for_closure = window.clone();
    let closure = wasm_bindgen::closure::Closure::<dyn FnMut()>::wrap(Box::new(move || {
        let detector = detector.clone();
        let detect_fn = detect_fn.clone();
        let stream = stream.clone();
        let window = window_for_closure.clone();
        tick.set(tick.get() + 1);
        // Query the current element synchronously (it may have been recreated
        // by a re-render) and re-bind the stream if it lost it.
        let Some(video) = window
            .document()
            .and_then(|d| d.get_element_by_id(VIDEO_ID))
            .map(|el| el.unchecked_into::<web_sys::HtmlVideoElement>())
        else {
            return;
        };
        // Throttled heartbeat, surfaced on the scan status line (phones have
        // no console) so the camera state is visible while scanning.
        if tick.get().is_multiple_of(30) {
            model.app.scan_status.set(format!(
                "Camera: ready={} paused={} bound={} — point at the QR.",
                video.ready_state(),
                video.paused(),
                video.src_object().is_some()
            ));
        }
        if video.src_object().is_none() {
            video.set_src_object(Some(&stream));
            let _ = video.play();
        }
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

/// Accumulate a decoded string: an invite link joins the event directly; a
/// chunked frame joins the session (and triggers import once complete); a
/// whole parcel imports directly.
fn handle_scan_string(model: Model, text: &str) {
    let complete_invite = crate::event::Invite::from_url(text).filter(|inv| {
        !inv.homeserver.is_empty()
            && !inv.event.is_empty()
            && !inv.sid.is_empty()
            && !inv.tid.is_empty()
    });
    if let Some(inv) = complete_invite {
        model.app.scan_active.set(false);
        stop_scan();
        crate::update(model, crate::Msg::Join(inv));
        return;
    }
    if let Ok(frame) = crate::services::qr::unpack_frame(text) {
        let complete = SCAN.with(|sc| {
            let mut b = sc.borrow_mut();
            let Some(sess) = b.as_mut() else {
                return false;
            };
            if sess.seen.insert((frame.id.clone(), frame.index)) {
                sess.frames.push(frame.clone());
                // Complete only when every frame is present AND it inflates to a
                // real parcel (compressed payload).
                crate::services::qr::frames_to_parcel(&sess.frames).is_ok()
            } else {
                false
            }
        });
        if complete {
            let parcel = SCAN.with(|sc| {
                sc.borrow()
                    .as_ref()
                    .and_then(|s| crate::services::qr::frames_to_parcel(&s.frames).ok())
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
