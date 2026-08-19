//! Camera QR scanning for parcel import (wasm only).
//!
//! Uses the browser's `BarcodeDetector` (Chrome/Edge/Android) over a live
//! `getUserMedia` video feed.  Falls back to a jsQR shim when `BarcodeDetector`
//! is missing (Brave, Firefox).  Decoded `khanatime_qr:` / `khanatime_parcel:`
//! strings are accumulated; once a chunked parcel's frames are all present the
//! parcel is re-joined and imported through the same path as pasting
//! (`sync::import_parcel_text`).

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
    model.sync.scan_active.set(true);
    model.sync.scan_status.set("Starting camera…".to_string());
    wasm_bindgen_futures::spawn_local(async move {
        run_scan(model).await;
    });
}

/// Start the camera scanner in preview mode: detected text is stored in
/// `scan_preview` instead of being auto-imported.  Used by the QR page to show
/// a confirmation modal before importing.
pub fn start_scan_preview(model: Model) {
    model.sync.scan_preview.set(None);
    stop_scan();
    model.sync.scan_active.set(true);
    model
        .sync
        .scan_status
        .set("Scanning — point at a QR code…".to_string());
    wasm_bindgen_futures::spawn_local(async move {
        run_scan(model).await;
    });
}

async fn run_scan(model: Model) {
    let Some(window) = web_sys::window() else {
        model.sync.scan_active.set(false);
        return;
    };

    // Feature-detect BarcodeDetector, fall back to jsQR shim.
    let use_jsqr;
    let detector;
    let detect_fn;

    match js_sys::Reflect::get(&window, &"BarcodeDetector".into()) {
        Ok(v) if !v.is_undefined() && !v.is_null() => {
            // Native BarcodeDetector available (Chrome/Edge/Android).
            use_jsqr = false;
            let opts = js_sys::Object::new();
            let formats = js_sys::Array::of1(&"qr_code".into());
            let _ = js_sys::Reflect::set(&opts, &"formats".into(), &formats);
            let ctor_fn: js_sys::Function = v.unchecked_into();
            match js_sys::Reflect::construct(&ctor_fn, &js_sys::Array::of1(&opts)) {
                Ok(d) => {
                    detector = d;
                    detect_fn = js_sys::Reflect::get(&detector, &"detect".into())
                        .unwrap()
                        .unchecked_into();
                }
                Err(_) => {
                    model.sync.scan_active.set(false);
                    model.sync.scan_status.set(
                        "QR scanning unavailable here — paste the parcel instead.".to_string(),
                    );
                    return;
                }
            }
        }
        _ => {
            // BarcodeDetector missing — check for jsQR shim (Brave, Firefox).
            match js_sys::Reflect::get(&window, &"scanQRFromVideo".into()) {
                Ok(v) if !v.is_undefined() && !v.is_null() => {
                    use_jsqr = true;
                    detector = JsValue::undefined();
                    detect_fn = v.unchecked_into();
                }
                _ => {
                    model.sync.scan_active.set(false);
                    model.sync.scan_status.set(
                        "QR scanning not supported in this browser — paste the parcel instead."
                            .to_string(),
                    );
                    return;
                }
            }
        }
    }

    let nav = window.navigator();
    let Ok(media) = nav.media_devices() else {
        model.sync.scan_active.set(false);
        model
            .sync
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
                model.sync.scan_active.set(false);
                model
                    .sync
                    .scan_status
                    .set("Camera permission denied.".to_string());
                return;
            }
        },
        Err(_) => {
            model.sync.scan_active.set(false);
            model
                .sync
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
        model.sync.scan_active.set(false);
        model
            .sync
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
            .sync
            .scan_status
            .set(format!("Camera start failed: {e:?} — check permission."));
    } else if n == 0 {
        model
            .sync
            .scan_status
            .set("Camera has no video track — try the other camera.".to_string());
    } else {
        let backend = if use_jsqr { "jsQR" } else { "native" };
        model.sync.scan_status.set(format!(
            "Camera on ({n} track{s}, {backend}) — point at the QR.",
            s = if n == 1 { "" } else { "s" }
        ));
    }

    let interval = spawn_detect_loop(model, detector, detect_fn, stream.clone(), use_jsqr);
    SCAN.with(|s| {
        *s.borrow_mut() = Some(ScanSession {
            stream,
            interval,
            seen: Default::default(),
            frames: Vec::new(),
        })
    });
    model.sync.scan_active.set(true);
    model
        .sync
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
    use_jsqr: bool,
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
            model.sync.scan_status.set(format!(
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
            if use_jsqr {
                // jsQR shim path: call window.scanQRFromVideo(video) → string|null.
                let arg = video.into();
                let Ok(result) = detect_fn.call1(&JsValue::undefined(), &arg) else {
                    return;
                };
                if let Some(text) = result.as_string() {
                    if !text.is_empty() {
                        handle_scan_string(model, &text);
                    }
                }
            } else {
                // Native BarcodeDetector path: detector.detect(video) → array of results.
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
    // Preview mode: scan_preview is Some("") sentinel from start_scan_preview.
    // Store the detected text and stop so the QR page can show confirmation.
    if model
        .sync
        .scan_preview
        .with(|p| p.as_ref().map(String::is_empty).unwrap_or(false))
    {
        model.sync.scan_preview.set(Some(text.to_string()));
        model.sync.scan_active.set(false);
        stop_scan();
        return;
    }
    // Try parsing as a URL or bare query string for typed imports.
    let params: std::collections::HashMap<String, String> = {
        let query = if let Ok(url) = url::Url::parse(text) {
            url.query().unwrap_or("").to_string()
        } else {
            text.strip_prefix('?').unwrap_or(text).to_string()
        };
        url::form_urlencoded::parse(query.as_bytes())
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect()
    };
    if let Some(ty) = params.get("type").map(String::as_str) {
        match ty {
            "account" => {
                if let (Some(homeserver), Some(user_id)) =
                    (params.get("homeserver"), params.get("user_id"))
                {
                    let password = params.get("password").cloned().unwrap_or_default();
                    model.sync.scan_active.set(false);
                    stop_scan();
                    crate::update(
                        model,
                        crate::Msg::ImportAccount {
                            homeserver: homeserver.clone(),
                            user_id: user_id.clone(),
                            password,
                        },
                    );
                    return;
                }
            }
            "contact" => {
                if let Some(user_id) = params.get("user_id") {
                    let name = params.get("name").cloned().unwrap_or_default();
                    let description = params.get("description").cloned().unwrap_or_default();
                    let phone = params.get("phone").cloned().filter(|s| !s.is_empty());
                    model.sync.scan_active.set(false);
                    stop_scan();
                    crate::update(
                        model,
                        crate::Msg::ImportContact {
                            user_id: user_id.clone(),
                            name,
                            description,
                            phone,
                        },
                    );
                    return;
                }
            }
            _ => {}
        }
    }
    let complete_invite = crate::event::Invite::from_url(text).filter(|inv| {
        !inv.homeserver.is_empty()
            && !inv.event.is_empty()
            && !inv.sid.is_empty()
            && !inv.tid.is_empty()
    });
    if let Some(inv) = complete_invite {
        model.sync.scan_active.set(false);
        stop_scan();
        crate::update(model, crate::Msg::Join(inv));
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
    model.sync.scan_active.set(false);
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
