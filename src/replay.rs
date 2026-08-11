//! Pure reconstruction of in-memory event state from a message log.
//!
//! The event, scores and run records are derived state: rebuilt by replaying
//! the durable message log (room history + pending outbox) in order.  Room
//! history replays oldest→newest, then pending (locally created, unsent)
//! messages, so a device's own offline entries win over stale room backfill.

use crate::event::{EventInfo, RunRecord, ScoreData};
use crate::log::LogMsg;
use crate::timing_event::TimingEvent;

/// Reconstruct `(event, scores, runs)` from the event's message log.
pub fn replay(log: &[LogMsg], pending: &[LogMsg]) -> (EventInfo, Vec<ScoreData>, Vec<RunRecord>) {
    let mut ev = EventInfo::default();
    let mut scores: Vec<ScoreData> = vec![];
    let mut runs: Vec<RunRecord> = vec![];
    for msg in log.iter().chain(pending.iter()) {
        apply(&mut ev, &mut scores, &mut runs, &msg.body);
    }
    (ev, scores, runs)
}

/// Apply one message body to derived state (idempotent: setup is
/// last-writer-wins, runs dedupe by record, scores overwrite per stage+car).
pub fn apply(
    ev: &mut EventInfo,
    scores: &mut Vec<ScoreData>,
    runs: &mut Vec<RunRecord>,
    body: &str,
) {
    if body.starts_with(TimingEvent::SETUP_PREFIX) {
        if let Some(incoming) = crate::event::from_setup_body(body) {
            crate::event::merge_setup(ev, &incoming);
        }
        return;
    }
    let Some(te) = TimingEvent::from_body(body) else {
        return; // plain chat / results snapshot: no state
    };
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EntryStatus, EventStatus, KTime, KTimeTime};
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
        TimingEvent {
            r#type: r#type.into(),
            event_id: event_id.into(),
            test,
            car: car.into(),
            run,
            ts,
            time_ds: Some(time_ds),
            status: Some("clean".into()),
            flags: Some(0),
            official_id: None,
        }
    }

    fn room(msg_ts: i64, body: String) -> LogMsg {
        LogMsg::from_room(
            format!("!{msg_ts}"),
            msg_ts,
            "alice".into(),
            body,
            String::new(),
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
            room(200, te("start", "kt-2026-demo", 1, "7", 1, 150, 0).body()),
            room(
                300,
                te("finish", "kt-2026-demo", 1, "7", 1, 280, 1234).body(),
            ),
        ];
        let (ev2, scores, runs) = replay(&log, &[]);
        assert_eq!(ev2.id, "kt-2026-demo");
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
            te("finish", "kt-2026-demo", 1, "7", 1, 280, 9999).body(),
        )];
        let pending = vec![pend(
            400,
            te("finish", "kt-2026-demo", 1, "7", 1, 390, 1234).body(),
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
        let body = te("finish", "kt-2026-demo", 1, "7", 1, 280, 1234).body();
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
        let mut a = base_event();
        let mut b = base_event();
        b.id = "kt-2026-other".into();
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
}
