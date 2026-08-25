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
    pub r#type: String, // start | stop | finish | amend | void
    /// Event uid (the wire identity, not the human slug).
    pub event_id: String,
    /// This observation's id — the indelible thing.
    pub uid: String,
    /// amend/void: the corrected observation's uid.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    pub test: u8,
    pub car: String,
    pub ts: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_ds: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub official_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// UIDs of the start/stop observations this finish is based on (audit trail).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refs: Vec<String>,

    // ---- signing (added at source, transport never touches) ----
    /// Base64 Ed25519 public key of the device that created this observation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signing_key: Option<String>,
    /// Base64 Ed25519 signature of the canonical payload (all fields except
    /// `signing_key` and `signature`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
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

    /// Body prefix of a signed hello message (associates signing key with Matrix ID).
    pub const HELLO_PREFIX: &'static str = "khanatime_hello:";

    pub fn new(r#type: &str, event_id: &str, test: u8, car: &str) -> Self {
        Self {
            r#type: r#type.to_string(),
            event_id: event_id.to_string(),
            uid: crate::ids::gen_short_id(),
            target: None,
            test,
            car: car.to_string(),
            ts: js_sys::Date::now() as i64,
            time_ds: None,
            status: None,
            flags: None,
            official_id: None,
            comment: None,
            refs: vec![],
            signing_key: None,
            signature: None,
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

    /// A `start` payload.
    pub fn start(event_id: &str, test: u8, car: &str) -> Self {
        Self::new("start", event_id, test, car)
    }

    /// A `stop` payload (lightweight off-course status with elapsed time).
    pub fn stop(event_id: &str, test: u8, car: &str, time_ds: u16) -> Self {
        let mut te = Self::new("stop", event_id, test, car);
        te.time_ds = Some(time_ds);
        te
    }

    /// A `finish` payload for an entered time, referencing the contributing observations.
    pub fn finish(
        event_id: &str,
        test: u8,
        car: &str,
        time: &crate::event::KTime,
        refs: Vec<String>,
    ) -> Self {
        let mut te = Self::new("finish", event_id, test, car);
        te.apply_time(time);
        te.refs = refs;
        te
    }

    /// Amend an existing observation: a fresh message targeting `target` with
    /// corrected fields.  The original stays in the log; replay patches it.
    pub fn amend(
        event_id: &str,
        target: &str,
        test: u8,
        car: &str,
        time: &crate::event::KTime,
    ) -> Self {
        let mut te = Self::new("amend", event_id, test, car);
        te.target = Some(target.to_string());
        te.apply_time(time);
        te
    }

    /// Void an existing observation by `target` uid.  Final — if wrong, enter
    /// a fresh observation.
    pub fn void(event_id: &str, target: &str, test: u8, car: &str) -> Self {
        let mut te = Self::new("void", event_id, test, car);
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

    /// Sign this timing event with the device key.  Sets `signing_key` and
    /// `signature` fields.
    pub fn sign_with(
        &mut self,
        device_keys: &crate::signing::DeviceKeys,
    ) -> Result<(), crate::signing::SigningError> {
        let (sig, key) = crate::signing::sign_payload(self, device_keys)?;
        self.signature = Some(sig);
        self.signing_key = Some(key);
        Ok(())
    }

    /// Verify this timing event's signature.  Returns Ok if valid, Err if
    /// invalid or unsigned.
    pub fn verify_signature(&self) -> Result<(), crate::signing::SigningError> {
        let sig = self
            .signature
            .as_ref()
            .ok_or(crate::signing::SigningError::NoPrivateKey)?;
        let key = self
            .signing_key
            .as_ref()
            .ok_or(crate::signing::SigningError::NoPrivateKey)?;
        crate::signing::verify_payload(self, sig, key)
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
            ts: 0,
            time_ds: None,
            status: None,
            flags: None,
            official_id: None,
            comment: None,
            refs: vec![],
            signing_key: None,
            signature: None,
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

    #[test]
    fn refs_omitted_when_empty() {
        let te = base();
        let json = serde_json::to_value(&te).unwrap();
        assert!(!json.as_object().unwrap().contains_key("refs"));
    }

    #[test]
    fn refs_present_when_nonempty() {
        let mut te = base();
        te.refs = vec!["abc".into(), "def".into()];
        let json = serde_json::to_value(&te).unwrap();
        let refs = json["refs"].as_array().unwrap();
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].as_str(), Some("abc"));
    }
}
