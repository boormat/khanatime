use serde::{Deserialize, Serialize};

/// Wire format for timing events exchanged over Matrix.
///
/// Payload carried as the `khanatime` content key of an `m.room.message`
/// event in the `#timing` room (see `docs/research/MessagingSpike.md`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimingEvent {
    pub r#type: String, // start | finish | penalty | result
    pub event_id: String,
    pub test: u8,
    pub car: String,
    pub run: u8,
    pub ts: i64,
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

    pub fn new(r#type: &str, event_id: &str, test: u8, car: &str, run: u8) -> Self {
        Self {
            r#type: r#type.to_string(),
            event_id: event_id.to_string(),
            test,
            car: car.to_string(),
            run,
            ts: js_sys::Date::now() as i64,
            status: None,
            flags: None,
            official_id: None,
        }
    }

    /// Wrap this event as `m.room.message` content.
    pub fn to_matrix_content(&self) -> serde_json::Value {
        let json = serde_json::to_value(self).expect("timing event serializes");
        serde_json::json!({
            "msgtype": "m.text",
            "body": format!("KT {}", json),
            Self::CONTENT_KEY: json,
        })
    }

    /// Extract a [TimingEvent] from an `m.room.message` content map.
    pub fn from_matrix_content(content: &serde_json::Value) -> Option<Self> {
        serde_json::from_value(content.get(Self::CONTENT_KEY)?.clone()).ok()
    }
}
