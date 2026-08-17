use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const ENTRY_STATUSES: &[(&str, &str)] = &[
    ("draft", "Draft Entry"),
    ("submitted", "Entry Submitted"),
    ("accepted", "Accepted"),
    ("reserve", "Reserve"),
    ("confirmed", "Confirmed"),
    ("started", "Started"),
    ("withdrawn", "Withdrawn"),
];

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum EntryStatus {
    Draft,
    #[default]
    Submitted,
    Accepted,
    Reserve,
    Confirmed,
    Started,
    Withdrawn,
}

impl EntryStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntryStatus::Draft => "draft",
            EntryStatus::Submitted => "submitted",
            EntryStatus::Accepted => "accepted",
            EntryStatus::Reserve => "reserve",
            EntryStatus::Confirmed => "confirmed",
            EntryStatus::Started => "started",
            EntryStatus::Withdrawn => "withdrawn",
        }
    }
}

impl std::fmt::Display for EntryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            EntryStatus::Draft => "draft entry",
            EntryStatus::Submitted => "entry submitted",
            EntryStatus::Accepted => "accepted",
            EntryStatus::Reserve => "reserve",
            EntryStatus::Confirmed => "confirmed",
            EntryStatus::Started => "started",
            EntryStatus::Withdrawn => "withdrawn",
        };
        write!(f, "{s}")
    }
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum EventStatus {
    #[default]
    Draft,
    Published,
    Running,
    Finished,
}

impl std::fmt::Display for EventStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            EventStatus::Draft => "draft",
            EventStatus::Published => "published",
            EventStatus::Running => "running",
            EventStatus::Finished => "finished",
        };
        write!(f, "{s}")
    }
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Entry {
    #[serde(default)]
    pub entry_no: u32,
    pub car: String,
    #[serde(default)]
    pub preferred_car: String,
    pub name: String,
    #[serde(default)]
    pub vehicle: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub shared_car: Option<String>,
    #[serde(default)]
    pub order: u32,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub licence: Option<String>,
    #[serde(default)]
    pub passenger: Option<String>,
    #[serde(default)]
    pub status: EntryStatus,
    #[serde(default)]
    pub owner: Option<String>,
}

impl Entry {
    pub fn new(name: &str, car: &str) -> Self {
        Self {
            name: name.to_string(),
            car: car.to_string(),
            ..Default::default()
        }
    }
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct EntryEvent {
    pub id: String,
    pub uid: String,
    pub name: String,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub entries: Vec<Entry>,
    #[serde(default)]
    pub status: EventStatus,
    #[serde(default)]
    pub entries_enabled: bool,
    #[serde(default)]
    pub homeserver: String,
    #[serde(default)]
    pub space_id: Option<String>,
    #[serde(default)]
    pub timing_id: Option<String>,
    #[serde(default)]
    pub space_alias: Option<String>,
}

impl EntryEvent {
    pub fn find_entry(&self, entry_no: u32) -> Option<&Entry> {
        self.entries.iter().find(|e| e.entry_no == entry_no)
    }

    pub fn remove_entry(&mut self, entry_no: u32) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.entry_no != entry_no);
        self.entries.len() != len
    }

    pub fn upsert_entry(&mut self, entry: Entry) -> bool {
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|e| e.entry_no == entry.entry_no)
        {
            *existing = entry;
            false
        } else {
            self.entries.push(entry);
            true
        }
    }

    pub fn is_published(&self) -> bool {
        self.status != EventStatus::Draft
    }

    pub fn is_demo(&self) -> bool {
        self.id == "demo-training"
    }
}

pub fn entry_sort_key(e: &Entry) -> (bool, u32, u32) {
    let active = matches!(
        e.status,
        EntryStatus::Submitted
            | EntryStatus::Accepted
            | EntryStatus::Confirmed
            | EntryStatus::Started
    );
    (!active, e.order, e.entry_no)
}

pub fn shared_entry_nos(entries: &[Entry]) -> HashSet<u32> {
    let mut map: std::collections::HashMap<&str, Vec<u32>> = std::collections::HashMap::new();
    for e in entries {
        if let Some(sh) = &e.shared_car {
            let key = sh.trim();
            if !key.is_empty() {
                map.entry(key).or_default().push(e.entry_no);
            }
        }
    }
    map.values()
        .filter(|v| v.len() > 1)
        .flatten()
        .copied()
        .collect()
}

pub fn normalize_car_number(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
    let letters: String = trimmed
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if digits.is_empty() && letters.is_empty() {
        return Err("Invalid car number".to_string());
    }
    Ok(format!("{digits}{letters}").trim().to_string())
}

pub fn suggest_car_number(used: &HashSet<String>, preferred: &str) -> String {
    let preferred = normalize_car_number(preferred).unwrap_or_default();
    if !preferred.is_empty() && !used.contains(&preferred) {
        return preferred;
    }
    for n in 1..=9999 {
        let candidate = n.to_string();
        if !used.contains(&candidate) {
            return candidate;
        }
    }
    "???".to_string()
}
