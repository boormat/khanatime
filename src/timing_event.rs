use serde::{Deserialize, Serialize};

/// Wire format for timing events exchanged over Matrix.
///
/// Payload carried as the `khanatime` content key of an `m.room.message`
/// event in the event's `timing` room (see `docs/research/MessagingSpike.md`).
///
/// Every observation carries a generated `uid` — the indelible record.  A
/// correction is a *new* message (`amend`/`void`) that targets an existing
/// observation's `uid`; the original is never rewritten or removed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimingEvent {
    pub r#type: String, // start | finish | amend | void
    /// Event uid (the wire identity, not the human slug).
    pub event_id: String,
    /// This observation's id — the indelible thing.
    pub uid: String,
    /// amend/void: the corrected observation's uid.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    pub test: u8,
    pub car: String,
    pub run: u8,
    pub ts: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_ds: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub official_id: Option<String>,
}

impl TimingEvent {
    /// Matrix event type used to carry timing payloads.
    pub const MESSAGE_TYPE: &'static str = "m.room.message";

    /// Content key that marks a message as a timing payload.
    pub const CONTENT_KEY: &'static str = "khanatime";

    /// Body prefix of an event-setup message (carries a serialized [crate::event::EventInfo]).
    pub const SETUP_PREFIX: &'static str = "khanatime_setup:";

    /// Body prefix of a results-snapshot message (informational / audit only).
    pub const RESULT_PREFIX: &'static str = "khanatime_result:";

    /// Body prefix of a per-entry state message (carries a serialized
    /// [crate::event::EntryMsg]).
    pub const ENTRY_PREFIX: &'static str = "khanatime_entry:";

    pub fn new(r#type: &str, event_id: &str, test: u8, car: &str, run: u8) -> Self {
        Self {
            r#type: r#type.to_string(),
            event_id: event_id.to_string(),
            uid: crate::ids::gen_short_id(),
            target: None,
            test,
            car: car.to_string(),
            run,
            ts: js_sys::Date::now() as i64,
            time_ds: None,
            status: None,
            flags: None,
            official_id: None,
        }
    }

    /// Fill the finish payload fields from an entered [crate::event::KTime].
    fn apply_time(&mut self, time: &crate::event::KTime) {
        match time {
            crate::event::KTime::Time(t) => {
                self.flags = Some(t.flags);
                self.time_ds = Some(t.time_ds);
                self.status = if t.garage {
                    Some("garage".to_string())
                } else {
                    Some("clean".to_string())
                };
            }
            crate::event::KTime::DNF => self.status = Some("dnf".to_string()),
            crate::event::KTime::FTS => self.status = Some("fts".to_string()),
            crate::event::KTime::WD => self.status = Some("wd".to_string()),
            crate::event::KTime::NOSHO => self.status = Some("nosho".to_string()),
        }
    }

    /// A `finish` payload for an entered time.
    pub fn finish(
        event_id: &str,
        stage: u8,
        car: &str,
        run: u8,
        time: &crate::event::KTime,
    ) -> Self {
        let mut te = Self::new("finish", event_id, stage, car, run);
        te.apply_time(time);
        te
    }

    /// Amend an existing observation: a fresh message targeting `target` with
    /// corrected fields.  The original stays in the log; replay patches it.
    pub fn amend(
        event_id: &str,
        target: &str,
        test: u8,
        car: &str,
        run: u8,
        time: &crate::event::KTime,
    ) -> Self {
        let mut te = Self::new("amend", event_id, test, car, run);
        te.target = Some(target.to_string());
        te.apply_time(time);
        te
    }

    /// Void an existing observation by `target` uid.  Final — if wrong, enter
    /// a fresh observation.
    pub fn void(event_id: &str, target: &str, test: u8, car: &str, run: u8) -> Self {
        let mut te = Self::new("void", event_id, test, car, run);
        te.target = Some(target.to_string());
        te
    }

    /// The `m.text` body this event is carried in (also stored in pending).
    pub fn body(&self) -> String {
        format!(
            "KT {}",
            serde_json::to_value(self).expect("timing event serializes")
        )
    }

    /// Parse a `KT {json}` message body back into a [TimingEvent].
    pub fn from_body(body: &str) -> Option<Self> {
        let json = body.strip_prefix("KT ")?;
        serde_json::from_str(json).ok()
    }

    /// Wrap this event as `m.room.message` content.
    pub fn to_matrix_content(&self) -> serde_json::Value {
        serde_json::json!({
            "msgtype": "m.text",
            "body": self.body(),
            Self::CONTENT_KEY: serde_json::to_value(self).expect("timing event serializes"),
        })
    }

    /// Extract a [TimingEvent] from an `m.room.message` content map.
    pub fn from_matrix_content(content: &serde_json::Value) -> Option<Self> {
        serde_json::from_value(content.get(Self::CONTENT_KEY)?.clone()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{KTime, KTimeTime};

    fn base() -> TimingEvent {
        TimingEvent {
            r#type: "finish".into(),
            event_id: "ev".into(),
            uid: "ABCDEFGHJK".into(),
            target: None,
            test: 1,
            car: "7".into(),
            run: 2,
            ts: 0,
            time_ds: None,
            status: None,
            flags: None,
            official_id: None,
        }
    }

    #[test]
    fn clean_time() {
        let mut te = base();
        te.apply_time(&KTime::Time(KTimeTime {
            time_ds: 123,
            flags: 1,
            garage: false,
        }));
        assert_eq!(te.status.as_deref(), Some("clean"));
        assert_eq!(te.time_ds, Some(123));
        assert_eq!(te.flags, Some(1));
    }

    #[test]
    fn garage_time() {
        let mut te = base();
        te.apply_time(&KTime::Time(KTimeTime {
            time_ds: 50,
            flags: 0,
            garage: true,
        }));
        assert_eq!(te.status.as_deref(), Some("garage"));
        assert_eq!(te.time_ds, Some(50));
    }

    #[test]
    fn dnf() {
        let mut te = base();
        te.apply_time(&KTime::DNF);
        assert_eq!(te.status.as_deref(), Some("dnf"));
        assert_eq!(te.time_ds, None);
    }

    #[test]
    fn roundtrip_via_content() {
        let mut te = base();
        te.apply_time(&KTime::Time(KTimeTime {
            time_ds: 321,
            flags: 2,
            garage: false,
        }));
        let content = te.to_matrix_content();
        assert_eq!(TimingEvent::from_matrix_content(&content), Some(te));
    }
}
