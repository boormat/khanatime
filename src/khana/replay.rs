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
///
/// `scores` is always re-derived from `runs` at the end (see
/// [`scores_from_runs`]) so it can never drift from the runs — in particular a
/// `void` (which clears `r.voided` but leaves the run in place) drops the
/// scored time, and a corrected/amended finish always reflects the latest
/// intent.  The incremental scoring in `apply` was removed on purpose: the log
/// is the truth, `runs` is a deduplicated projection of it, and `scores` is a
/// pure function of `runs`.
///
/// Observations and setup manifests are accepted only when their signature
/// verifies (see [`crate::signing::verdict_with`]): rejected messages stay in
/// the durable log but never enter derived state, so a bogus setup simply leaves
/// the last valid one in place.
pub fn replay(log: &[LogMsg], pending: &[LogMsg]) -> (EventInfo, Vec<ScoreData>, Vec<RunRecord>) {
    let mut ev = EventInfo::default();
    let mut runs: Vec<RunRecord> = vec![];
    // Amend/void that arrived before their target (QR parcel ordering); applied
    // when the targeted observation lands.
    let mut corrections: HashMap<String, Vec<TimingEvent>> = HashMap::new();
    // Load the trust registry once for the whole replay.
    let reg = crate::signing::SigningKeyRegistry::load();
    for msg in log.iter().chain(pending.iter()) {
        apply(&mut ev, &mut runs, &mut corrections, &reg, &msg.body);
    }
    let scores = scores_from_runs(&runs);
    (ev, scores, runs)
}

/// Re-derive the per-(stage, car) score table from the run records.  Voided
/// runs are skipped, so a voided finish removes the time from the results (the
/// run itself stays in the log for audit).  Last-writer-wins: a later run with
/// the same uid overwrote the earlier one in `add_run`, so this is
/// order-independent.
pub(crate) fn scores_from_runs(runs: &[RunRecord]) -> Vec<ScoreData> {
    let mut scores: Vec<ScoreData> = vec![];
    for r in runs.iter().filter(|r| !r.voided) {
        if r.r#type == crate::event::RUN_START && r.status.as_deref() == Some("dns") {
            crate::event::upsert_ktime(&mut scores, r.test, &r.car, crate::event::KTime::NOSHO);
        }
        if r.r#type == crate::event::RUN_FINISH {
            let kt = crate::event::finish_to_ktime(r);
            crate::event::upsert_ktime(&mut scores, r.test, &r.car, kt);
        }
    }
    scores
}

/// Apply one message body to derived state (idempotent: setup is
/// last-writer-wins, runs dedupe by observation uid).  Only messages that pass
/// [`crate::signing::verdict_with`] enter derived state; rejected ones are left
/// for the durable log.  `scores` are NOT updated here — they are rebuilt from
/// `runs` by [`replay`].
fn apply(
    ev: &mut EventInfo,
    runs: &mut Vec<RunRecord>,
    corrections: &mut HashMap<String, Vec<TimingEvent>>,
    reg: &crate::signing::SigningKeyRegistry,
    body: &str,
) {
    if body.starts_with(TimingEvent::SETUP_PREFIX) {
        if let Some(incoming) = crate::event::from_setup_body(body) {
            let verdict = crate::signing::verdict_with(
                &incoming,
                incoming.signature.as_ref(),
                incoming.signing_key.as_ref(),
                reg,
            );
            // Ignore a bogus/unsigned setup so the last valid one stays in place.
            if crate::signing::accepted(&verdict) {
                crate::event::merge_setup(ev, &incoming);
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
    if !crate::signing::accepted(&crate::signing::verdict_with(
        &te,
        te.signature.as_ref(),
        te.signing_key.as_ref(),
        reg,
    )) {
        return; // unsigned / invalid / rejected: keep in log, don't build state
    }
    if te.r#type == crate::event::RUN_START
        || te.r#type == crate::event::RUN_FINISH
        || te.r#type == crate::event::RUN_STOP
    {
        let run = crate::event::record_from_timing(&te);
        crate::event::add_run(runs, run);
    }
    // A newly landed observation may satisfy a stashed amend/void (QR ordering).
    retry_corrections(runs, corrections);
    if te.r#type == "amend" || te.r#type == "void" {
        apply_correction(runs, corrections, &te);
    }
}

/// Apply an `amend`/`void` message to its targeted observation.  When the
/// target isn't here yet the message is stashed; it's retried when the
/// observation lands (see [retry_corrections]).
fn apply_correction(
    runs: &mut [RunRecord],
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
    patch_target(runs, target, te);
    retry_corrections(runs, corrections);
}

/// Patch (or void) the targeted observation in place.  `scores` are rebuilt
/// from `runs` by [`replay`], so this only mutates the run record.
fn patch_target(runs: &mut [RunRecord], target: &str, te: &TimingEvent) {
    let Some(r) = runs.iter_mut().find(|r| r.uid == target) else {
        return;
    };
    if te.r#type == "void" {
        r.voided = true;
        return;
    }
    r.test = te.test;
    r.car = te.car.clone();
    r.time_ds = te.time_ds;
    r.status = te.status.clone();
    r.flags = te.flags;
    r.official_id = te.official_id.clone();
    r.comment = te.comment.clone();
}

/// Apply any stashed corrections whose target has since arrived.
fn retry_corrections(runs: &mut [RunRecord], corrections: &mut HashMap<String, Vec<TimingEvent>>) {
    let pending: Vec<TimingEvent> = corrections.drain().flat_map(|(_, v)| v).collect();
    for te in pending {
        if let Some(target) = te.target.as_deref() {
            if crate::event::find_run(runs, target).is_some() {
                patch_target(runs, target, &te);
            } else {
                corrections.entry(target.to_string()).or_default().push(te);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EventStatus, KTime, KTimeTime};
    use crate::log::LogMsg;

    /// Use the real signing setup_body so setup manifests arrive signed (TOFU =
    /// accepted) — the trust gate rejects unsigned setups now.
    fn setup_body(ev: &EventInfo) -> String {
        crate::event::setup_body(ev)
    }

    /// Build an observation, signed with a throwaway device key (TOFU = Unknown).
    /// Real observations are signed, so the trust gate accepts them.
    fn te(r#type: &str, event_id: &str, test: u8, car: &str, ts: i64, time_ds: u16) -> TimingEvent {
        let t = r#type;
        let mut te = TimingEvent {
            r#type: t.into(),
            event_id: event_id.into(),
            uid: format!("uid-{t}-{ts}"),
            target: None,
            test,
            car: car.into(),
            ts,
            time_ds: Some(time_ds),
            status: Some("clean".into()),
            flags: Some(0),
            official_id: None,
            comment: None,
            refs: vec![],
            signing_key: None,
            signature: None,
        };
        let keys = crate::signing::DeviceKeys::generate();
        te.sign_with(&keys).expect("sign");
        te
    }

    /// Build an **unsigned** observation (used only in gate-rejection tests).
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
            room(200, te("start", "ev-uid-demo", 1, "7", 150, 0).body()),
            room(300, te("finish", "ev-uid-demo", 1, "7", 280, 1234).body()),
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
            te("finish", "ev-uid-demo", 1, "7", 280, 9999).body(),
        )];
        let pending = vec![pend(
            400,
            te("finish", "ev-uid-demo", 1, "7", 390, 1234).body(),
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
        let body = te("finish", "ev-uid-demo", 1, "7", 280, 1234).body();
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
    }

    #[allow(clippy::too_many_arguments)]
    fn obs_te(
        r#type: &str,
        event_id: &str,
        uid: &str,
        target: Option<&str>,
        test: u8,
        car: &str,
        ts: i64,
        time_ds: Option<u16>,
    ) -> TimingEvent {
        let mut te = raw_obs_te(r#type, event_id, uid, target, test, car, ts, time_ds);
        let keys = crate::signing::DeviceKeys::generate();
        te.sign_with(&keys).expect("sign");
        te
    }

    /// Build an **unsigned** observation (used only in gate-rejection tests).
    #[allow(clippy::too_many_arguments)]
    fn raw_obs_te(
        r#type: &str,
        event_id: &str,
        uid: &str,
        target: Option<&str>,
        test: u8,
        car: &str,
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
            ts,
            time_ds,
            status: Some("clean".into()),
            flags: Some(0),
            official_id: None,
            comment: None,
            refs: vec![],
            signing_key: None,
            signature: None,
        }
    }

    /// Signed observation with an explicit status (e.g. "dns").
    #[allow(clippy::too_many_arguments)]
    fn signed_obs_te_status(
        r#type: &str,
        event_id: &str,
        uid: &str,
        target: Option<&str>,
        test: u8,
        car: &str,
        ts: i64,
        time_ds: Option<u16>,
        status: &str,
    ) -> String {
        let mut te = raw_obs_te(r#type, event_id, uid, target, test, car, ts, time_ds);
        te.status = Some(status.into());
        let keys = crate::signing::DeviceKeys::generate();
        te.sign_with(&keys).expect("sign");
        te.body()
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
                obs_te("start", "ev-uid-demo", "s1", None, 1, "7", 200, None).body(),
            ),
            room(
                300,
                obs_te("finish", "ev-uid-demo", "f1", None, 1, "7", 400, Some(800)).body(),
            ),
            room(
                400,
                obs_te("void", "ev-uid-demo", "v1", Some("f1"), 1, "7", 450, None).body(),
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
    fn void_drops_the_scored_time() {
        // Regression: a voided finish must not leave a phantom time in `scores`
        // (it was previously updated only on amend, not void — see #6 review).
        let ev = base_event();
        let log = vec![
            room(100, setup_body(&ev)),
            room(
                200,
                obs_te("finish", "ev-uid-demo", "f1", None, 1, "7", 400, Some(800)).body(),
            ),
            room(
                300,
                obs_te("void", "ev-uid-demo", "v1", Some("f1"), 1, "7", 450, None).body(),
            ),
        ];
        let (_, scores, runs) = replay(&log, &[]);
        assert!(runs.iter().find(|r| r.uid == "f1").unwrap().voided);
        assert!(scores.is_empty(), "voided finish must not keep a score");
    }

    #[test]
    fn dns_start_scores_nosho() {
        let ev = base_event();
        let log = vec![
            room(100, setup_body(&ev)),
            room(
                200,
                obs_te("start", "ev-uid-demo", "s1", None, 1, "7", 200, None).body(),
            ),
            room(
                300,
                signed_obs_te_status("start", "ev-uid-demo", "s2", None, 1, "7", 205, None, "dns"),
            ),
        ];
        let (_, scores, _) = replay(&log, &[]);
        assert_eq!(scores.len(), 1);
        assert_eq!(scores[0].time, KTime::NOSHO);
    }

    #[test]
    fn manual_dns_finish_scores_nosho() {
        // B8: a DNS recorded through the manual edit path is a finish with
        // status "dns" and must score NOSHO (mirrors the DNS-start case).
        let ev = base_event();
        let log = vec![
            room(100, setup_body(&ev)),
            room(
                200,
                signed_obs_te_status(
                    "finish",
                    "ev-uid-demo",
                    "f1",
                    None,
                    1,
                    "7",
                    300,
                    Some(400),
                    "dns",
                ),
            ),
        ];
        let (_, scores, _) = replay(&log, &[]);
        assert_eq!(scores.len(), 1);
        assert_eq!(scores[0].time, KTime::NOSHO);
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
            280,
            Some(1234),
        )
        .body();
        let log = vec![room(300, body.clone()), room(400, body)];
        let (_, _, runs) = replay(&log, &[]);
        assert_eq!(runs.len(), 1);
    }

    #[test]
    fn unsigned_observations_are_rejected() {
        // With default-deny, an unsigned finish must not produce a run or score.
        let ev = base_event();
        let log = vec![
            room(100, setup_body(&ev)),
            room(
                200,
                raw_obs_te("finish", "ev-uid-demo", "f1", None, 1, "7", 400, Some(800)).body(),
            ),
        ];
        let (_, scores, runs) = replay(&log, &[]);
        assert!(runs.is_empty(), "unsigned finish must not build a run");
        assert!(scores.is_empty(), "unsigned finish must not score");
    }

    #[test]
    fn tampered_observation_is_rejected() {
        let ev = base_event();
        // Sign a finish, then change the body so the signature no longer matches.
        let mut te = raw_obs_te("finish", "ev-uid-demo", "f1", None, 1, "7", 400, Some(800));
        let keys = crate::signing::DeviceKeys::generate();
        te.sign_with(&keys).expect("sign");
        let bad = te
            .body()
            .replace("\"time_ds\":800", "\"time_ds\":999")
            .replace("\"uid\":\"f1\"", "\"uid\":\"f9\"");
        let log = vec![room(100, setup_body(&ev)), room(200, bad)];
        let (_, scores, runs) = replay(&log, &[]);
        assert!(runs.is_empty(), "tampered finish must not build a run");
        assert!(scores.is_empty());
    }

    #[test]
    fn signed_observation_is_accepted() {
        let ev = base_event();
        let log = vec![
            room(100, setup_body(&ev)),
            room(
                200,
                obs_te("finish", "ev-uid-demo", "f1", None, 1, "7", 400, Some(800)).body(),
            ),
        ];
        let (_, scores, runs) = replay(&log, &[]);
        assert_eq!(runs.len(), 1);
        assert_eq!(scores.len(), 1);
        assert_eq!(
            scores[0].time,
            crate::event::KTime::Time(crate::event::KTimeTime {
                time_ds: 800,
                flags: 0,
                garage: false
            })
        );
    }

    #[test]
    fn bogus_setup_ignored_last_valid_wins() {
        // A bogus (unsigned) setup must not replace a previously accepted one, and
        // must not blank the event.
        let good = base_event(); // setup_body signs it
        let bogus = EventInfo {
            id: good.id.clone(),
            name: "Hijacked".into(),
            status: crate::event::EventStatus::Running,
            ..Default::default()
        };
        let bogus_body = format!(
            "{}{}",
            crate::timing_event::TimingEvent::SETUP_PREFIX,
            serde_json::to_string(&bogus).unwrap()
        );

        // Bogus arrives after the good setup — good one should remain.
        let log = vec![room(100, setup_body(&good)), room(200, bogus_body.clone())];
        let (ev, _, _) = replay(&log, &[]);
        assert_eq!(
            ev.name, "Demo",
            "bogus setup must not override the valid one"
        );

        // Bogus arrives before the good setup — good one should still win.
        let log = vec![room(100, bogus_body), room(200, setup_body(&good))];
        let (ev, _, _) = replay(&log, &[]);
        assert_eq!(ev.name, "Demo");
    }
}
