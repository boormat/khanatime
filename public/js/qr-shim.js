// qr-shim.js — jsQR fallback for browsers without BarcodeDetector.
// Called from WASM (qr_scan.rs) via js_sys when the native API is missing.
//
// window.scanQRFromVideo(video) -> string|null
//   Captures the current frame from a <video> element and runs jsQR on it.
//   Returns the decoded QR string, or null if no QR code is visible.
window.scanQRFromVideo = function(video) {
  if (!video || !video.videoWidth || !video.videoHeight) return null;
  if (typeof jsQR === 'undefined') return null;
  var canvas = document.createElement('canvas');
  canvas.width = video.videoWidth;
  canvas.height = video.videoHeight;
  var ctx = canvas.getContext('2d');
  if (!ctx) return null;
  ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
  var imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
  var code = jsQR(imageData.data, imageData.width, imageData.height);
  return code ? code.data : null;
};
