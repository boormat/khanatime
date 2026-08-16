//! Pure reconstruction of in-memory event state from a message log.
//!
//! The event, scores and run records are derived state: rebuilt by replaying
//! the durable message log (room history + pending outbox) in order.  Room
//! history replays oldest→newest, then pending (locally created, unsent)
//! messages, so a device's own offline entries win over stale room backfill.
//!
//! Every observation carries a generated `uid` (the wire identity, see
//! `timing_event.rs`).  A correction is a new `amend`/`void` message targeting
//! an observation's `uid`; the original stays in the log, and derived state
//! reflects the latest intent.

use std::collections::HashMap;

use crate::event::{EventInfo, RunRecord, ScoreData};
use crate::log::LogMsg;
use crate::timing_event::TimingEvent;

/// Reconstruct `(event, scores, runs)` from the event's message log.
pub fn replay(log: &[LogMsg], pending: &[LogMsg]) -> (EventInfo, Vec<ScoreData>, Vec<RunRecord>) {
    let mut ev = EventInfo::default();
    let mut scores: Vec<ScoreData> = vec![];
    let mut runs: Vec<RunRecord> = vec![];
    // Amend/void that arrived before their target (QR parcel ordering); applied
    // when the targeted observation lands.
    let mut corrections: HashMap<String, Vec<TimingEvent>> = HashMap::new();
    for msg in log.iter().chain(pending.iter()) {
        apply(&mut ev, &mut scores, &mut runs, &mut corrections, &msg.body);
    }
    (ev, scores, runs)
}

/// Apply one message body to derived state (idempotent: setup is
/// last-writer-wins, runs dedupe by observation uid, scores overwrite per
/// stage+car).
fn apply(
    ev: &mut EventInfo,
    scores: &mut Vec<ScoreData>,
    runs: &mut Vec<RunRecord>,
    corrections: &mut HashMap<String, Vec<TimingEvent>>,
    body: &str,
) {
    if body.starts_with(TimingEvent::SETUP_PREFIX) {
        if let Some(incoming) = crate::event::from_setup_body(body) {
            crate::event::merge_setup(ev, &incoming);
        }
        return;
    }
    if body.starts_with(TimingEvent::ENTRY_PREFIX) {
        if let Some(msg) = crate::event::from_entry_body(body) {
            // Like setup: a fresh device adopts the message's event uid; once
            // an event is adopted, other events' entry messages are skipped.
            if ev.uid.is_empty() {
                ev.uid = msg.event_id.clone();
            }
            if msg.event_id == ev.uid {
                if msg.delete {
                    ev.remove_entry(msg.entry.entry_no);
                } else {
                    ev.upsert_entry(msg.entry);
                }
            }
        }
        return;
    }
    let Some(te) = TimingEvent::from_body(body) else {
        return; // plain chat / results snapshot: no state
    };
    // Adoption: the first timing message for this event establishes the uid;
    // everything else scoped to that uid.
    if ev.uid.is_empty() {
        ev.uid = te.event_id.clone();
    }
    if te.event_id != ev.uid {
        return;
    }
    if te.r#type == crate::event::RUN_START || te.r#type == crate::event::RUN_FINISH {
        let run = crate::event::record_from_timing(&te);
        crate::event::add_run(runs, run);
    }
    if te.r#type == crate::event::RUN_START && te.status.as_deref() == Some("dns") {
        // A no-show start scores NOSHO so the results cell reads "DNS".
        crate::event::upsert_ktime(scores, te.test, &te.car, crate::event::KTime::NOSHO);
    }
    if te.r#type == crate::event::RUN_FINISH {
        let run = crate::event::record_from_timing(&te);
        let kt = crate::event::finish_to_ktime(&run);
        crate::event::upsert_ktime(scores, te.test, &te.car, kt);
    }
    // A newly landed observation may satisfy a stashed amend/void (QR ordering).
    retry_corrections(runs, scores, corrections);
    if te.r#type == "amend" || te.r#type == "void" {
        apply_correction(runs, scores, corrections, &te);
    }
}

/// Apply an `amend`/`void` message to its targeted observation.  When the
/// target isn't here yet the message is stashed; it's retried when the
/// observation lands (see [retry_corrections]).
fn apply_correction(
    runs: &mut [RunRecord],
    scores: &mut Vec<ScoreData>,
    corrections: &mut HashMap<String, Vec<TimingEvent>>,
    te: &TimingEvent,
) {
    let Some(target) = te.target.as_deref() else {
        return;
    };
    if !crate::event::find_run(runs, target).is_some() {
        corrections
            .entry(target.to_string())
            .or_default()
            .push(te.clone());
        return;
    }
    patch_target(runs, scores, target, te);
    retry_corrections(runs, scores, corrections);
}

/// Patch (or void) the targeted observation, then refresh that stage's score
/// from the runs so results reflect the latest intent.
fn patch_target(
    runs: &mut [RunRecord],
    scores: &mut Vec<ScoreData>,
    target: &str,
    te: &TimingEvent,
) {
    let Some(r) = runs.iter_mut().find(|r| r.uid == target) else {
        return;
    };
    if te.r#type == "void" {
        r.voided = true;
        return;
    }
    r.test = te.test;
    r.car = te.car.clone();
    r.run = te.run;
    r.time_ds = te.time_ds;
    r.status = te.status.clone();
    r.flags = te.flags;
    r.official_id = te.official_id.clone();
    r.comment = te.comment.clone();
    if r.r#type == crate::event::RUN_FINISH {
        let kt = crate::event::finish_to_ktime(r);
        crate::event::upsert_ktime(scores, te.test, &te.car, kt);
    }
}

/// Apply any stashed corrections whose target has since arrived.
fn retry_corrections(
    runs: &mut [RunRecord],
    scores: &mut Vec<ScoreData>,
    corrections: &mut HashMap<String, Vec<TimingEvent>>,
) {
    let pending: Vec<TimingEvent> = corrections.drain().flat_map(|(_, v)| v).collect();
    for te in pending {
        if let Some(target) = te.target.as_deref() {
            if crate::event::find_run(runs, target).is_some() {
                patch_target(runs, scores, target, &te);
            } else {
                corrections.entry(target.to_string()).or_default().push(te);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Entry, EntryStatus, EventStatus, KTime, KTimeTime};
    use crate::log::LogMsg;

    fn setup_body(ev: &EventInfo) -> String {
        format!(
            "{}{}",
            TimingEvent::SETUP_PREFIX,
            serde_json::to_string(ev).unwrap()
        )
    }

    fn te(
        r#type: &str,
        event_id: &str,
        test: u8,
        car: &str,
        run: u8,
        ts: i64,
        time_ds: u16,
    ) -> TimingEvent {
        let t = r#type;
        TimingEvent {
            r#type: t.into(),
            event_id: event_id.into(),
            uid: format!("uid-{t}-{ts}"),
            target: None,
            test,
            car: car.into(),
            run,
            ts,
            time_ds: Some(time_ds),
            status: Some("clean".into()),
            flags: Some(0),
            official_id: None,
            comment: None,
        }
    }

    fn room(msg_ts: i64, body: String) -> LogMsg {
        LogMsg::from_room(
            format!("!{msg_ts}"),
            msg_ts,
            "alice".into(),
            body,
            String::new(),
            "!room",
        )
    }

    fn pend(msg_ts: i64, body: String) -> LogMsg {
        LogMsg::new_pending(body, "me".into()).with_ts(msg_ts)
    }

    trait WithTs {
        fn with_ts(self, ts: i64) -> Self;
    }
    impl WithTs for LogMsg {
        fn with_ts(mut self, ts: i64) -> Self {
            self.ts = ts;
            self
        }
    }

    fn base_event() -> EventInfo {
        EventInfo {
            uid: "ev-uid-demo".into(),
            id: "kt-2026-demo".into(),
            name: "Demo".into(),
            ..Default::default()
        }
    }

    #[test]
    fn replay_reconstructs_event_scores_runs() {
        let mut ev = base_event();
        ev.add_entry("7", "Alice");
        ev.status = EventStatus::Running;
        let log = vec![
            room(100, setup_body(&ev)),
            room(200, te("start", "ev-uid-demo", 1, "7", 1, 150, 0).body()),
            room(
                300,
                te("finish", "ev-uid-demo", 1, "7", 1, 280, 1234).body(),
            ),
        ];
        let (ev2, scores, runs) = replay(&log, &[]);
        assert_eq!(ev2.id, "kt-2026-demo");
        assert_eq!(ev2.uid, "ev-uid-demo");
        assert_eq!(ev2.name, "Demo");
        assert_eq!(ev2.status, EventStatus::Running);
        assert_eq!(ev2.entries.len(), 1);
        assert_eq!(scores.len(), 1);
        assert_eq!(
            scores[0].time,
            KTime::Time(KTimeTime {
                time_ds: 1234,
                flags: 0,
                garage: false
            })
        );
        assert_eq!(runs.len(), 2);
    }

    #[test]
    fn pending_wins_over_stale_remote() {
        let log = vec![room(
            300,
            te("finish", "ev-uid-demo", 1, "7", 1, 280, 9999).body(),
        )];
        let pending = vec![pend(
            400,
            te("finish", "ev-uid-demo", 1, "7", 1, 390, 1234).body(),
        )];
        let (_, scores, _) = replay(&log, &pending);
        assert_eq!(scores.len(), 1);
        assert_eq!(
            scores[0].time,
            KTime::Time(KTimeTime {
                time_ds: 1234,
                flags: 0,
                garage: false
            })
        );
    }

    #[test]
    fn duplicate_runs_collapse() {
        let body = te("finish", "ev-uid-demo", 1, "7", 1, 280, 1234).body();
        let log = vec![room(300, body.clone())];
        let pending = vec![pend(300, body.clone())];
        let (_, _, runs) = replay(&log, &pending);
        assert_eq!(runs.len(), 1);
    }

    #[test]
    fn setup_last_writer_wins_same_id() {
        let mut a = base_event();
        a.name = "One".into();
        let mut b = base_event();
        b.name = "Two".into();
        let log = vec![room(100, setup_body(&a)), room(200, setup_body(&b))];
        let (ev, _, _) = replay(&log, &[]);
        assert_eq!(ev.name, "Two");
    }

    #[test]
    fn setup_other_event_id_skipped_after_adoption() {
        let a = base_event();
        let mut b = base_event();
        b.id = "kt-2026-other".into();
        b.uid = "ev-uid-other".into();
        b.name = "Other".into();
        let log = vec![room(100, setup_body(&a)), room(200, setup_body(&b))];
        let (out, _, _) = replay(&log, &[]);
        assert_eq!(out.id, "kt-2026-demo"); // adopted the first setup
        assert_eq!(out.name, "Demo"); // the other event's setup was skipped
    }

    #[test]
    fn plain_chat_and_result_are_state_noops() {
        let log = vec![
            room(100, "hello".into()),
            room(200, format!("{} {{}}", TimingEvent::RESULT_PREFIX)),
        ];
        let (ev, scores, runs) = replay(&log, &[]);
        assert!(ev.is_null());
        assert!(scores.is_empty());
        assert!(runs.is_empty());
    }

    #[test]
    fn pending_setup_seeds_fresh_event() {
        let mut ev = base_event();
        ev.add_entry("1", "Sam");
        let pending = vec![pend(100, setup_body(&ev))];
        let (out, _, _) = replay(&[], &pending);
        assert_eq!(out.id, "kt-2026-demo");
        assert_eq!(out.entries.len(), 1);
        assert_eq!(out.entries[0].status, EntryStatus::Submitted);
    }

    fn entry_msg(event_id: &str, entry: &crate::event::Entry, delete: bool) -> String {
        crate::event::entry_body(event_id, entry, delete)
    }

    #[test]
    fn entry_upsert_and_tombstone() {
        let mut ev = base_event();
        ev.add_entry("7", "Alice");
        ev.add_entry("9", "Dan");
        let mut confirmed = ev.entries.iter().find(|e| e.car == "7").unwrap().clone();
        confirmed.status = EntryStatus::Confirmed;
        // Tombstone must carry the correct entry_no so remove_entry finds it.
        let mut tombstone = Entry::new("9", "Dan");
        tombstone.entry_no = ev.entries.iter().find(|e| e.car == "9").unwrap().entry_no;
        let log = vec![
            room(100, setup_body(&ev)),
            room(200, entry_msg("ev-uid-demo", &confirmed, false)),
            room(300, entry_msg("ev-uid-demo", &tombstone, true)),
        ];
        let (out, _, _) = replay(&log, &[]);
        assert_eq!(out.entries.len(), 1);
        assert_eq!(out.entries[0].status, EntryStatus::Confirmed);
        assert!(out.entries.iter().all(|e| e.car != "9"));
    }

    #[test]
    fn entry_other_event_skipped() {
        let mut ev = base_event();
        ev.add_entry("7", "Alice");
        let log = vec![
            room(100, setup_body(&ev)),
            room(
                200,
                entry_msg("ev-uid-other", &Entry::new("42", "Stranger"), false),
            ),
        ];
        let (out, _, _) = replay(&log, &[]);
        assert_eq!(out.uid, "ev-uid-demo");
        assert_eq!(out.entries.len(), 1);
    }

    #[test]
    fn entry_seeds_fresh_event() {
        let log = vec![room(
            100,
            entry_msg("ev-uid-fresh", &Entry::new("7", "Alice"), false),
        )];
        let (out, _, _) = replay(&log, &[]);
        assert_eq!(out.uid, "ev-uid-fresh");
        assert_eq!(out.entries.len(), 1);
    }

    #[allow(clippy::too_many_arguments)]
    fn obs_te(
        r#type: &str,
        event_id: &str,
        uid: &str,
        target: Option<&str>,
        test: u8,
        car: &str,
        run: u8,
        ts: i64,
        time_ds: Option<u16>,
    ) -> TimingEvent {
        TimingEvent {
            r#type: r#type.into(),
            event_id: event_id.into(),
            uid: uid.into(),
            target: target.map(str::to_string),
            test,
            car: car.into(),
            run,
            ts,
            time_ds,
            status: Some("clean".into()),
            flags: Some(0),
            official_id: None,
            comment: None,
        }
    }

    #[test]
    fn amend_patches_the_target_run() {
        let ev = base_event();
        let log = vec![
            room(100, setup_body(&ev)),
            room(
                200,
                obs_te(
                    "finish",
                    "ev-uid-demo",
                    "obs-1",
                    None,
                    1,
                    "7",
                    1,
                    280,
                    Some(1234),
                )
                .body(),
            ),
            room(
                300,
                obs_te(
                    "amend",
                    "ev-uid-demo",
                    "amd-1",
                    Some("obs-1"),
                    1,
                    "7",
                    1,
                    300,
                    Some(999),
                )
                .body(),
            ),
        ];
        let (_, scores, runs) = replay(&log, &[]);
        let r = runs
            .iter()
            .find(|r| r.uid == "obs-1")
            .expect("original stays");
        assert_eq!(r.time_ds, Some(999));
        assert!(!r.voided);
        assert_eq!(runs.len(), 1); // amend is not a new run
        assert_eq!(
            scores[0].time,
            KTime::Time(KTimeTime {
                time_ds: 999,
                flags: 0,
                garage: false
            })
        );
    }

    #[test]
    fn void_excludes_from_pending_starts() {
        use crate::event::pending_starts;
        let ev = base_event();
        let log = vec![
            room(100, setup_body(&ev)),
            room(
                200,
                obs_te("start", "ev-uid-demo", "s1", None, 1, "7", 1, 200, None).body(),
            ),
            room(
                300,
                obs_te(
                    "finish",
                    "ev-uid-demo",
                    "f1",
                    None,
                    1,
                    "7",
                    1,
                    400,
                    Some(800),
                )
                .body(),
            ),
            room(
                400,
                obs_te(
                    "void",
                    "ev-uid-demo",
                    "v1",
                    Some("f1"),
                    1,
                    "7",
                    1,
                    450,
                    None,
                )
                .body(),
            ),
        ];
        let (_, _, runs) = replay(&log, &[]);
        assert!(runs.iter().find(|r| r.uid == "f1").unwrap().voided);
        assert!(!runs.iter().find(|r| r.uid == "s1").unwrap().voided);
        // The voided finish no longer pairs: car 7's start is pending again.
        let pending = pending_starts(&runs, 1);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].car, "7");
    }

    #[test]
    fn amend_before_target_applies_when_target_lands() {
        let ev = base_event();
        let log = vec![
            room(100, setup_body(&ev)),
            room(
                200,
                obs_te(
                    "amend",
                    "ev-uid-demo",
                    "amd-1",
                    Some("obs-1"),
                    1,
                    "7",
                    1,
                    200,
                    Some(555),
                )
                .body(),
            ),
            room(
                300,
                obs_te(
                    "finish",
                    "ev-uid-demo",
                    "obs-1",
                    None,
                    1,
                    "7",
                    1,
                    250,
                    Some(1111),
                )
                .body(),
            ),
        ];
        let (_, scores, runs) = replay(&log, &[]);
        let r = runs
            .iter()
            .find(|r| r.uid == "obs-1")
            .expect("target landed");
        assert_eq!(r.time_ds, Some(555)); // stashed amend applied on arrival
        assert_eq!(
            scores[0].time,
            KTime::Time(KTimeTime {
                time_ds: 555,
                flags: 0,
                garage: false
            })
        );
    }

    #[test]
    fn same_uid_via_two_log_entries_collapses() {
        let body = obs_te(
            "finish",
            "ev-uid-demo",
            "obs-1",
            None,
            1,
            "7",
            1,
            280,
            Some(1234),
        )
        .body();
        let log = vec![room(300, body.clone()), room(400, body)];
        let (_, _, runs) = replay(&log, &[]);
        assert_eq!(runs.len(), 1);
    }
}
