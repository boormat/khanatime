# Car Photos — MXC Reference Approach

> Status: future change, not yet implemented.
> See also: `docs/plan/` for other planned features.
>
> **Stale references (2026-08-18):** This document references `EntryMsg`
> struct which has been removed from event.rs.

## Goal

Allow officials to see a small photo of each car on the timing screens (car chips,
pending confirmations, results). Photos are stored on the Matrix media repository
and referenced by MXC URI, not embedded in the event data.

## Architecture

### Upload path

1. A dedicated **photo upload room** per event (created alongside the timing room),
   or photos are uploaded to the event's chat room.
2. An official posts a photo with a caption like `#17` or `Alice` to identify the car.
3. The app parses the caption, matches it to an entry, and records the MXC URI on
   the `Entry` record.
4. The updated entry is published as a `khanatime_entry:` message (last-writer-wins).

### Storage

- `Entry.photo: Option<String>` — the MXC URI (e.g. `mxc://matrix.org/abc123`).
- NOT base64 in the JSON. The entry stays ~230 bytes.
- Photo bytes live on the Matrix media repository, fetched on demand.

### Retrieval

- UI renders a car chip → checks if `entry.photo` is `Some`.
- If yes: `GET /_matrix/media/v3/download/{server}/{media_id}` → image bytes.
- Cache in IndexedDB for offline/repeat views.
- No need to pull the chat log — the MXC URI is all you need.

### Image processing

- Client-side resize on upload: crop to car, downscale to 48×48px, export as WebP.
- Fallback: PNG if WebP encoding isn't available.
- Background removal is optional (nice-to-have, not required).

## Data model changes

```rust
pub struct Entry {
    // ... existing fields ...
    /// MXC URI of the car photo (uploaded to the event's photo room).
    #[serde(default)]
    pub photo: Option<String>,
}
```

### Entry message

```rust
pub struct EntryMsg {
    pub event_id: String,
    pub ts: i64,
    pub entry: Entry,  // now includes photo: Option<String>
    pub delete: bool,
}
```

No changes to `TimingEvent` or `RunRecord`.

## Transport impact

| Scenario | Without photos | With photos (MXC) | With photos (base64) |
|----------|---------------|-------------------|---------------------|
| Entry JSON | ~220 B | ~230 B | ~3 KB |
| 30-car event | ~6.4 KB | ~6.7 KB | ~87 KB |
| Matrix event size limit | OK | OK | Tight (65 KB) |
| QR parcel (DEFLATE) | ~3 KB | ~3.5 KB | ~45 KB |

MXC references are negligible — the photo data stays on the server.

## UI integration

### Car chips (`pad::car_chips`)

- If entry has `photo`: render a 48×48 rounded thumbnail next to the car number/name.
- If no photo: current layout (text only).

### Big selected-car chip (`stopwatch::view_selected_car`)

- Show the photo as a 64×64 thumbnail beside the car number + driver name.

### Timing log (`page::view_timing_log`)

- Optionally show a 32×32 thumbnail inline with each log entry.

### Results (`page::results`)

- Show a 48×48 thumbnail in the car column.

## Upload flow

1. Official taps "Add photo" on an entry (in event config or a photo management screen).
2. File picker opens (camera or gallery on mobile, file dialog on desktop).
3. Client-side processing:
   - Read the image.
   - Crop to center (or let user drag-crop).
   - Resize to 48×48 (Lanczos).
   - Encode as WebP (or PNG fallback).
4. Upload to Matrix media repo: `PUT /_matrix/media/v3/upload` → get MXC URI.
5. Update the entry's `photo` field with the MXC URI.
6. Publish as `khanatime_entry:` message.

## Photo matching

When a photo is posted to the chat room with a caption:

1. Parse caption for car number (e.g. "#17", "17", "car 17") or driver name.
2. Match against `EventInfo.entries` by car number (preferred) or fuzzy name match.
3. If ambiguous: prompt the official to confirm which entry.
4. Record the MXC URI on the matched entry.

## Offline considerations

- If offline when photo is uploaded: queue the upload, retry on reconnect.
- Photos already fetched can be cached in IndexedDB for offline use.
- The MXC URI is small (~50 bytes) — storing it in localStorage is fine even
  if the photo hasn't been fetched yet.

## Implementation phases

1. **Phase 1**: Add `photo: Option<String>` to `Entry`. No upload UI yet. Existing
   events continue to work (photo defaults to `None`).
2. **Phase 2**: File picker + client-side resize + upload to Matrix media repo.
   Photo management screen in event config.
3. **Phase 3**: Auto-match photos posted to the chat room. Caption parsing.
4. **Phase 4**: Thumbnail rendering in car chips, timing log, results.

## Open questions

- Should photos be optional per entry, or required for all entries?
- Should there be a maximum file size (e.g. 100 KB after processing)?
- Should the photo room be a separate room per event, or reuse the timing room?
- Should officials be able to delete/replace photos?
- How to handle duplicate photos (same car posted twice)?
