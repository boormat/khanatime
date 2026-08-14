// Structure for in memory storage of event
// probably will do serialisation for long term storage

use std::collections::HashSet;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

pub const ID_PREFIX: &str = "kt-";

/// Id of the local-only demo event used to train officials.  Deliberately not
/// a valid event id (no `kt-` prefix), so it can never be published.
pub const DEMO_EVENT_ID: &str = "demo-training";
#[allow(dead_code)] // reserved for official roles on the publish/identity path
pub const ROLE_KEY_OFFICIAL: &str = "key_official";
pub const ROLE_OFFICIAL: &str = "official";
pub const ROLE_COMPETITOR: &str = "competitor";

// Run record types (mirrors the Matrix TimingEvent wire types).
pub const RUN_START: &str = "start";
pub const RUN_FINISH: &str = "finish";

/// One start or finish observation for a car on a test.  Persisted per event
/// under `runs:<id>` and exchanged over Matrix as a [TimingEvent].
#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct RunRecord {
    pub r#type: String, // RUN_START | RUN_FINISH
    pub test: u8,
    pub car: String,
    pub run: u8,
    pub ts: i64, // ms since epoch
    #[serde(default)]
    pub time_ds: Option<u16>,
    #[serde(default)]
    pub status: Option<String>, // "clean"|"dnf"|"fts"|"wd"|"garage"|"nosho"
    #[serde(default)]
    pub flags: Option<u8>,
    #[serde(default)]
    pub official_id: Option<String>,
}

/// Timing method for a test.  Stopwatch = lowest elapsed time wins.
/// Rally = the score is a target-style result (closest to target wins).
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TimingStyle {
    #[default]
    Stopwatch,
    Rally,
}

/// Configuration for one test (stage) in an event.
///
/// `num` gives the stage its display/ordering number.  `repeats` is the total
/// number of runs each car attempts (the Y in "best X of Y") and `best_x` is
/// how many of those count towards the score.  These are captured on the setup
/// page; the results engine currently uses one best score per car per test.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Stage {
    pub num: u8, // display/ordering number, e.g. 1..12
    #[serde(default)]
    pub name: String,
    /// Total runs per car per test (the Y in "best X of Y").
    #[serde(default = "default_one")]
    pub repeats: u8,
    /// Counted runs (the X in "best X of Y"; X <= repeats).
    #[serde(default = "default_one")]
    pub best_x: u8,
    #[serde(default)]
    pub timing: TimingStyle,
}

fn default_one() -> u8 {
    1
}

impl Default for Stage {
    fn default() -> Self {
        Stage::new(String::new(), 1, 1, TimingStyle::Stopwatch)
    }
}

impl Stage {
    pub fn new(name: String, repeats: u8, best_x: u8, timing: TimingStyle) -> Self {
        Self {
            num: 1,
            name,
            repeats,
            best_x,
            timing,
        }
    }

    /// A default stage for test `num`, as used when seeding a fresh event.
    pub fn for_test(num: u8) -> Self {
        let name = format!("Test {num}");
        Self {
            num,
            name,
            repeats: 1,
            best_x: 1,
            timing: TimingStyle::Stopwatch,
        }
    }
}

// Event INFO.  Staticish
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct EventInfo {
    pub name: String,

    /// Per-test configuration.  Authoritative for the number of tests.
    #[serde(default)]
    pub stages: Vec<Stage>,

    /// Legacy planned-stage count, read from pre-per-stage payloads only
    /// (never serialised back).  Migrated into `stages` by [Self::ensure_stages].
    #[serde(default, skip_serializing)]
    pub stages_count: u8,

    // scores: HashMap<i8, HashMap<String, CalcScore>>, // calculated for display.  Key is [stage][car] holding a Score.
    pub classes: Vec<String>, // list of known classes. Order as per display
    pub entries: Vec<Entry>,  // list of know entrants/drivers. Ordered by something

    // ---- draft / publish fields (set up front, editable later) ----
    // Stable primary key. Generated once at draft creation; renames never change it.
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub sponsoring_club: String,
    #[serde(default)]
    pub year: String,
    #[serde(default)]
    pub event_date: String,
    #[serde(default)]
    pub entry_open: String,
    #[serde(default)]
    pub entry_close: String,
    #[serde(default)]
    pub stripe_link: String,
    #[serde(default)]
    pub cost: String,
    #[serde(default)]
    pub max_entries: Option<u32>,
    #[serde(default)]
    pub info_links: Vec<String>,
    #[serde(default)]
    pub organisers: Vec<Official>,
    #[serde(default)]
    pub officials: Vec<Official>,
    #[serde(default)]
    pub best_x: u8,
    #[serde(default)]
    pub best_y: u8,
    #[serde(default)]
    pub status: EventStatus,

    // ---- Matrix (populated on publish) ----
    #[serde(default)]
    pub space_id: Option<String>,
    #[serde(default)]
    pub space_alias: Option<String>,
    #[serde(default)]
    pub timing_id: Option<String>,
    #[serde(default)]
    pub timing_alias: Option<String>,
}

/// Lifecycle of an entry in the event.
///
///   Draft -> Submitted -> Accepted -> Confirmed (paid) -> Started
///                    \-> Reserve (hold; promoted to Accepted when space opens)
///   Withdrawn can happen at any point.
#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum EntryStatus {
    /// On the entry list but not yet submitted.
    Draft,
    /// Entry submitted; being processed by the organisers.
    #[default]
    #[serde(alias = "entered")]
    Submitted,
    /// Processed and OK, awaiting payment to finalise.
    Accepted,
    /// Hold state when the event is full; promoted to Accepted when space opens.
    Reserve,
    /// Entry fee paid / entry confirmed.
    Confirmed,
    /// Competing (has started).
    Started,
    /// Withdrawn from the event entirely.
    Withdrawn,
}

impl EntryStatus {
    /// The serialized (lowercase, single-word) form used for storage and form values.
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
pub struct Entry {
    /// Stable primary key: per-event counter, assigned on creation (see
    /// [EventInfo::upsert_entry]).  Identity never changes, even when the car
    /// number does.  0 = not yet assigned.
    #[serde(default)]
    pub entry_no: u32,
    /// Assigned car number — "" until the timekeeper assigns one at
    /// close-entries.  Text, digits-first, uppercase (see
    /// [normalize_car_number]); timing data keys on this string.
    pub car: String,
    /// The car number the entrant nominated.  A preference only: may be
    /// blank, and duplicates are fine (the timekeeper resolves collisions).
    #[serde(default)]
    pub preferred_car: String,
    pub name: String, // name
    #[serde(default)]
    pub vehicle: String, // description
    /// Free-text shared-car name (rego, owner, description — whatever the
    /// entrant/timekeeper types).  Entries whose names match (see
    /// [shared_car_key]) share a physical car.  Informational only.
    #[serde(default)]
    pub shared_car: Option<String>,
    /// Running/display order, assigned at close-entries.  0 = unset (falls
    /// back to arrival order by `entry_no`).
    #[serde(default)]
    pub order: u32,
    #[serde(default)]
    pub classes: Vec<String>, // Classes. Count be an ID. meh
    #[serde(default)]
    pub licence: Option<String>,
    #[serde(default)]
    pub passenger: Option<String>,
    #[serde(default)]
    pub status: EntryStatus,
    /// Matrix user id of the person who entered themselves (empty for
    /// official-added entries).
    #[serde(default)]
    pub owner: Option<String>,
}

/// Wire format of a per-entry state message (`khanatime_entry:<json>`).
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct EntryMsg {
    pub event_id: String,
    pub ts: i64,
    /// Full entry snapshot (last-writer-wins per car).
    pub entry: Entry,
    /// Tombstone: remove the entry from the event.
    #[serde(default)]
    pub delete: bool,
}

/// A person attached to the event (organiser, official).
#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Official {
    #[serde(default)]
    pub id: String, // matrix user id, or "" for manual-only names
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub role: String, // ROLE_KEY_OFFICIAL | ROLE_OFFICIAL | ROLE_COMPETITOR
}

/// Lifecycle of the event.
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

#[derive(Default, PartialEq, Serialize, Deserialize, Debug, Clone)]
pub struct ScoreData {
    // keys For moment only accept int car numbers? 00 0B 24TBC
    pub stage: u8,

    pub car: String,
    pub time: KTime,
}

// #[derive(Copy, Clone, Default, Deserialize, PartialEq, Debug)]
#[derive(
    // parse_display::FromStr,
    // parse_display::Display,
    // Eq,
    PartialEq,
    Debug,
    Serialize,
    Deserialize,
    Default,
    Clone,
)]
// #[display("{time_ds/10.0} {flags}F {garage}G")]
pub struct KTimeTime {
    pub time_ds: u16,
    pub flags: u8,
    pub garage: bool,
}

#[allow(clippy::upper_case_acronyms)]
#[derive(
    // parse_display::FromStr,
    // parse_display::Display,
    PartialEq,
    Debug,
    Serialize,
    Deserialize,
    Default,
    Clone,
)]
// #[display("{}")]
pub enum KTime {
    #[default]
    NOSHO, // withdrawn, Did Not Start
    WD,
    FTS,
    DNF,
    // #[display("{0}")]
    Time(KTimeTime),
}

// NOTE Result ordering CAN change for classes.
// Maybe we should have a Display Score focussing on class? ie. regen after filter
// is selected.
// results to render
#[derive(Debug, Clone)]
pub struct ResultView {
    pub event: EventInfo,
    pub class: String,
    pub rows: IndexMap<u32, ResultRow>, // entries keyed by entry_no, in running order
    pub base_times_ds: Vec<u16>,        // base times

                                        // can probably remove the Index map so we can sort by a separate vec of refs?
}

// results to render
#[derive(Debug, Clone)]
pub struct ResultRow {
    pub entry: Entry, //todo use from context &'a [Entry];
    pub columns: Vec<Option<ResultScore>>,
    //cum_pos: Option<Pos>, // current/last cumulative position. None after a missed a stage
    // best-X-of-Y aggregate over the completed stages + its tie-aware rank
    pub total_ds: u32,
    pub total_pos: u8,
    pub total_eq: bool,
}

/// Results Position
///
#[derive(Default, Debug, Clone)]
pub struct Pos {
    pub score_ds: u16, // time in ds, after penalites
    pub pos: u8,       // cumulative pos in event. Not unique for equal times
    pub eq: bool,      // if pos is equal
    pub change: i8,    // delta of last stage (cumulative only?)
}

impl Pos {
    pub fn init(score_ds: u16) -> Self {
        Self {
            score_ds,
            pos: 0,
            eq: false,
            change: 0,
        }
    }
}

// Result for a Driver in a Stage
#[derive(Default, Clone, Debug)]
pub struct ResultScore {
    // raw result fields
    pub time: KTime, // as entered.. maybe an enum? of codes and time? pritable, so time plus penalties etc.
    pub stage_pos: Pos, // result within stage
    pub cum_pos: Option<Pos>, // pos in event.
}

//////////////////////////////////////////////////////////////////////
/// impl time
impl Default for EventInfo {
    fn default() -> Self {
        let classes = ["Outright", "Female", "Junior"];
        let classes = classes.map(String::from).into();
        let name = "".into();
        let stages: Vec<Stage> = (1..=3).map(Stage::for_test).collect();
        let entries = vec![];
        Self {
            name,
            stages,
            stages_count: 0,
            classes,
            entries,
            id: String::new(),
            sponsoring_club: String::new(),
            year: String::new(),
            event_date: String::new(),
            entry_open: String::new(),
            entry_close: String::new(),
            stripe_link: String::new(),
            cost: String::new(),
            max_entries: None,
            info_links: vec![],
            organisers: vec![],
            officials: vec![],
            best_x: 1,
            best_y: 1,
            status: EventStatus::Draft,
            space_id: None,
            space_alias: None,
            timing_id: None,
            timing_alias: None,
        }
    }
}

impl EventInfo {
    /// True when no event is currently selected (the null event).
    pub fn is_null(&self) -> bool {
        self.id.is_empty()
    }

    /// True for the local training event.  Demo events are never published and
    /// never join a timing room (see [demo_event]).
    pub fn is_demo(&self) -> bool {
        self.id.starts_with("demo-")
    }

    pub fn add_class(&mut self, class: &String) {
        if self.classes.contains(class) {
            return;
        }
        self.classes.push(class.clone());
    }

    /// Number of stages planned.
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    /// Per-stage configuration for test `idx` (0-based).
    pub fn stage(&self, idx: usize) -> Stage {
        self.stages.get(idx).cloned().unwrap_or_default()
    }

    /// Backfill per-stage configuration from the legacy global fields
    /// (`stages_count` / `best_x` / `best_y`).  No-op once `stages` is set.
    pub fn ensure_stages(&mut self) {
        if !self.stages.is_empty() {
            return;
        }
        let count = if self.stages_count > 0 {
            self.stages_count
        } else {
            3
        };
        self.stages = (1..=count)
            .map(|num| Stage {
                num,
                name: format!("Test {num}"),
                repeats: self.best_y.max(1),
                best_x: self.best_x,
                timing: TimingStyle::Stopwatch,
            })
            .collect();
    }

    // delete class, will ensure entries updated too
    pub fn remove_class(&mut self, class: &String) -> bool {
        if !self.classes.contains(class) {
            return false;
        }

        self.classes.retain(|x| x != class);
        for e in self.entries.iter_mut() {
            e.classes.retain(|x| x != class);
        }
        true
    }

    // delete class, will ensure entries updated too
    pub fn rename_class(&mut self, old: &str, new: &str) -> bool {
        if !self.classes.iter().any(|c| c == old) {
            return false;
        }

        let c: &mut String = self.classes.iter_mut().find(|x| *x == old).unwrap();
        *c = new.to_string();

        for e in self.entries.iter_mut() {
            if let Some(class) = e.classes.iter_mut().find(|x| *x == old) {
                *class = new.to_string();
            }
        }
        true
    }

    // delete class, will ensure entries updated too
    pub fn add_entry(&mut self, car: &str, name: &str) -> bool {
        let found_car = self.entries.iter().find(|e| e.car == *car).is_some();
        if found_car {
            return false;
        }

        // Dupe driver. ... is OK-ish?  Nah
        let found_driver = self.entries.iter().find(|e| e.name == *name).is_some();
        if found_driver {
            return false;
        }

        let mut entry = Entry::new(car, name);
        entry.entry_no = self.next_entry_no();
        self.entries.push(entry);
        true
    }

    /// The next unused entry number (counter, never reused within an event).
    pub fn next_entry_no(&self) -> u32 {
        self.entries.iter().map(|e| e.entry_no).max().unwrap_or(0) + 1
    }

    /// Find an entry by its stable entry number.
    pub fn find_entry(&self, entry_no: u32) -> Option<&Entry> {
        self.entries.iter().find(|e| e.entry_no == entry_no)
    }

    /// Find an entry by its assigned car number.
    pub fn find_entry_by_car(&self, car: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.car == car)
    }

    /// Entries in running/display order: explicit `order` first (0 = unset,
    /// sorted last in arrival order), ties broken by `entry_no`.
    pub fn sorted_entries(&self) -> Vec<&Entry> {
        let mut v: Vec<&Entry> = self.entries.iter().collect();
        v.sort_by_key(|e| entry_sort_key(e));
        v
    }

    /// Backfill entry numbers for legacy entries (entry_no 0).  Idempotent.
    pub fn ensure_entry_nos(&mut self) {
        let mut next = self.next_entry_no();
        for e in self.entries.iter_mut() {
            if e.entry_no == 0 {
                e.entry_no = next;
                next += 1;
            }
        }
    }

    /// Set the lifecycle status of an entry by entry number.
    pub fn set_entry_status(&mut self, entry_no: u32, status: EntryStatus) -> bool {
        if let Some(e) = self.entries.iter_mut().find(|e| e.entry_no == entry_no) {
            e.status = status;
            true
        } else {
            false
        }
    }

    // delete an entry by entry number
    pub fn remove_entry(&mut self, entry_no: u32) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.entry_no != entry_no);
        before != self.entries.len()
    }

    /// Insert or replace an entry (keyed by entry number; 0 is assigned the
    /// next number).  Returns true when the entry was new.
    ///
    /// Counter collision from concurrent offline creation on another device:
    /// when the existing entry with the same number clearly belongs to
    /// someone else (both have owners and they differ), the incoming entry is
    /// renumbered and appended instead of clobbering.
    pub fn upsert_entry(&mut self, entry: Entry) -> bool {
        let mut entry = entry;
        if entry.entry_no == 0 {
            entry.entry_no = self.next_entry_no();
        }
        match self
            .entries
            .iter()
            .position(|e| e.entry_no == entry.entry_no)
        {
            Some(i) => {
                let collision = self.entries[i].owner.is_some()
                    && entry.owner.is_some()
                    && self.entries[i].owner != entry.owner;
                if collision {
                    entry.entry_no = self.next_entry_no();
                    self.entries.push(entry);
                    true
                } else {
                    self.entries[i] = entry;
                    false
                }
            }
            None => {
                self.entries.push(entry);
                true
            }
        }
    }
}

/// Sort key for running/display order (see [EventInfo::sorted_entries]).
pub fn entry_sort_key(e: &Entry) -> (bool, u32, u32) {
    (e.order == 0, e.order, e.entry_no)
}

/// Encode an entry state message body (`khanatime_entry:<json>`).
pub fn entry_body(event_id: &str, entry: &Entry, delete: bool) -> String {
    let msg = EntryMsg {
        event_id: event_id.to_string(),
        ts: crate::log::now_ms(),
        entry: entry.clone(),
        delete,
    };
    format!(
        "{}{}",
        crate::timing_event::TimingEvent::ENTRY_PREFIX,
        serde_json::to_string(&msg).expect("entry msg serializes")
    )
}

/// Decode an entry state message body.  Returns None for other prefixes.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // wasm sync sink + tests
pub fn from_entry_body(body: &str) -> Option<EntryMsg> {
    let json = body.strip_prefix(crate::timing_event::TimingEvent::ENTRY_PREFIX)?;
    serde_json::from_str(json).ok()
}

impl Entry {
    pub fn new(car: &str, name: &str) -> Self {
        let vehicle = Default::default();
        let classes = ["Outright"];
        let classes = classes.map(String::from).into();
        let car = car.to_string();
        let name = name.to_string();
        Self {
            entry_no: 0, // assigned by EventInfo::add_entry/upsert_entry
            preferred_car: String::new(),
            vehicle,
            shared_car: None,
            order: 0,
            classes,
            car,
            name,
            licence: None,
            passenger: None,
            status: EntryStatus::Submitted,
            owner: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Car numbers.  Text, not integers: "007", "0", "00A" and "000" are all
// distinct.  Canonical form: no whitespace, uppercase, digits then optional
// letters (`^[0-9]+[A-Z]*$`).  The assigned number is what officials type at
// timing, so scores/runs key on it; the entry's identity is `entry_no`.
// ---------------------------------------------------------------------------

pub const CAR_NUMBER_MAX: usize = 8;

/// Validate a canonical car number: digits first, then optional uppercase
/// letters, no whitespace, length-capped.
pub fn validate_car_number(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err("Car number is empty".to_string());
    }
    if s.len() > CAR_NUMBER_MAX {
        return Err(format!("Car number too long (max {CAR_NUMBER_MAX} chars)"));
    }
    let mut seen_letter = false;
    for c in s.chars() {
        match c {
            'A'..='Z' => seen_letter = true,
            '0'..='9' if seen_letter => {
                return Err("Digits can't follow letters (e.g. 7A, not 7A2)".to_string())
            }
            '0'..='9' => {}
            _ => return Err(format!("Unexpected character '{c}' in car number")),
        }
    }
    if !s.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return Err("Car number must start with a digit".to_string());
    }
    Ok(())
}

/// Normalize a typed car number: strip all whitespace, uppercase, validate.
pub fn normalize_car_number(raw: &str) -> Result<String, String> {
    let s: String = raw
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase();
    validate_car_number(&s)?;
    Ok(s)
}

/// Maximum car number the suggestion algorithm will generate.  The pool is
/// deliberately small (club events rarely exceed 200 entries); see
/// [next_free_number] for the exhaustion fallback.
pub const MAX_SUGGESTED_NUMBER: u32 = 65_535;

/// Smallest positive integer not in `used` (as an exact string or numerically,
/// so "007" blocks "7").  Withdrawn entries still hold their numbers — a
/// number is never recycled within an event.
///
/// When the pool up to [MAX_SUGGESTED_NUMBER] is exhausted, falls back to
/// `next_entry_no + MAX_SUGGESTED_NUMBER` (never fails).
pub fn next_free_number(used: &std::collections::HashSet<String>) -> String {
    let nums: std::collections::HashSet<u32> = used.iter().filter_map(|c| c.parse().ok()).collect();
    for n in 1..=MAX_SUGGESTED_NUMBER {
        let s = n.to_string();
        if !used.contains(&s) && !nums.contains(&n) {
            return s;
        }
    }
    // Exhaustion (impossible in practice): generate a number outside the pool.
    let mut f = MAX_SUGGESTED_NUMBER + 1 + nums.len() as u32;
    while used.contains(&f.to_string()) || nums.contains(&f) {
        f += 1;
    }
    f.to_string()
}

/// Suggest an assigned car number: the entrant's preferred number when it's
/// valid and free, else the smallest free pure number.
pub fn suggest_car_number(used: &std::collections::HashSet<String>, preferred: &str) -> String {
    if !preferred.is_empty() && validate_car_number(preferred).is_ok() && !used.contains(preferred)
    {
        return preferred.to_string();
    }
    next_free_number(used)
}

// ---------------------------------------------------------------------------
// Shared cars.  `shared_car` is a typed name (rego, owner, description);
// entries whose names match share a physical car.  Informational only — it
// never affects numbering or timing, so it can change at any time.
// ---------------------------------------------------------------------------

/// Grouping key for a shared-car name: trimmed, single-spaced, lowercase.
pub fn shared_car_key(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Shared-car groups for display: `(display name, members)` for names shared
/// by ≥2 entries, members in running order.  First-seen casing wins for the
/// display name.
pub fn shared_groups(entries: &[Entry]) -> Vec<(String, Vec<&Entry>)> {
    let mut groups: IndexMap<String, (String, Vec<&Entry>)> = IndexMap::new();
    for e in entries {
        let Some(name) = e.shared_car.as_deref() else {
            continue;
        };
        let key = shared_car_key(name);
        if key.is_empty() {
            continue;
        }
        groups
            .entry(key)
            .or_insert_with(|| {
                (
                    name.split_whitespace().collect::<Vec<_>>().join(" "),
                    vec![],
                )
            })
            .1
            .push(e);
    }
    let mut out: Vec<(String, Vec<&Entry>)> = groups
        .into_values()
        .filter(|(_, members)| members.len() >= 2)
        .collect();
    for (_, members) in out.iter_mut() {
        members.sort_by_key(|e| entry_sort_key(e));
    }
    out
}

/// Entry numbers that share a physical car (members of a ≥2 group) — for
/// flagging shared cars on the timing screens.
pub fn shared_entry_nos(entries: &[Entry]) -> HashSet<u32> {
    shared_groups(entries)
        .iter()
        .flat_map(|(_, members)| members.iter().map(|e| e.entry_no))
        .collect()
}

impl<'a> ResultView {
    pub fn init(class: &str, event: &'a EventInfo, scores: &[ScoreData]) -> Self {
        let entries = find_entries_in_class(&event.entries, class);

        let rows: IndexMap<u32, ResultRow> = entries
            .iter()
            .map(|e| (e.entry_no, ResultRow::init(e, event, scores)))
            .collect();
        let class = class.to_string();

        let base_times_ds = vec![0; event.stage_count()];
        Self {
            class,
            event: event.clone(),
            rows,
            base_times_ds,
        }
    }
}

impl<'a> ResultRow {
    pub fn init(entry: &'a Entry, event: &'a EventInfo, scores: &[ScoreData]) -> Self {
        let columns = (0..event.stage_count())
            .map(|col| find_score(scores, &entry.car[..], col as u8 + 1).map(ResultScore::init))
            .collect();

        Self {
            entry: entry.clone(),
            columns,
            total_ds: 0,
            total_pos: 0,
            total_eq: false,
        }
    }
}

impl ResultScore {
    pub fn init(score: &ScoreData) -> Self {
        Self {
            time: score.time.clone(),
            stage_pos: Pos::default(),
            cum_pos: None,
        }
    }
}

impl KTimeTime {
    pub fn score_ds(&self) -> u32 {
        let flag_ds = 5 * 10u16; // 5 seconds
        let score = self.time_ds + (flag_ds * (self.flags as u16 + self.garage as u16));
        score as u32
    }
}

impl std::fmt::Display for KTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KTime::NOSHO => write!(f, "NOSHO"),
            KTime::WD => write!(f, "WD"),
            KTime::FTS => write!(f, "FTS"),
            KTime::DNF => write!(f, "DNF"),
            KTime::Time(t) => write!(
                f,
                "{} {}F {}G)",
                0.1f32 * t.time_ds as f32,
                t.flags,
                t.garage
            ),
        }
    }
}

// get base times for a stage
// calc base. min  min*2 max
pub fn calc_base_times(rv: &mut ResultView) {
    for stage in 0..rv.event.stage_count() {
        let mut fastest: u32 = u16::MAX as u32;
        let mut slowest: u32 = 0;
        for row in rv.rows.values() {
            if let Some(ResultScore {
                time: KTime::Time(kt),
                ..
            }) = &row.columns[stage]
            {
                // regs are unclear, but only thing that makes sense/fair
                // is the slowest time includes penalties.
                // (what is everyone got a penalty)
                fastest = fastest.min(kt.score_ds());
                slowest = slowest.max(kt.score_ds());
                // log!(stage + 1, fastest, slowest, kt.time_ds, row.entry.car);
            }
        }
        let base_time = slowest.min(fastest * 2);
        rv.base_times_ds[stage] = base_time as u16;
        // log!(
        //     "stage",
        //     stage + 1,
        //     "base time",
        //     base_time,
        //     "min",
        //     fastest,
        //     "max",
        //     slowest
        // );
    }
}

pub fn calc_penalties(rv: &mut ResultView) {
    for stage in 0..rv.event.stage_count() {
        for row in rv.rows.values_mut() {
            let base_time = rv.base_times_ds[stage];
            let plus10 = base_time + 100;
            let plus5 = base_time + 50;

            if let Some(rs) = &mut row.columns[stage] {
                let score_ds = match &rs.time {
                    KTime::NOSHO => plus10,
                    KTime::WD => plus5,
                    KTime::FTS => plus5,
                    KTime::DNF => plus5,
                    KTime::Time(t) => t.time_ds + (50u16 * (t.flags as u16 + t.garage as u16)),
                };
                rs.stage_pos.score_ds = score_ds;
            };
        }
    }
}

pub fn calc_cumulative_times(rv: &mut ResultView) {
    for row in rv.rows.values_mut() {
        let mut score = 0;
        for rs in row.columns.iter_mut().flatten() {
            score += rs.stage_pos.score_ds;
            rs.cum_pos = Some(Pos::init(score));
        }
    }
}

// helper to unpack nested cum_pos in ResultScore
fn get_cum_pos(rs: &mut Option<ResultScore>) -> Option<&mut Pos> {
    match rs {
        None => None,
        Some(rs) => match &mut rs.cum_pos {
            None => None,
            Some(pos) => Some(pos),
        },
    }
}

pub fn calc_pos_changes(rv: &mut ResultView) {
    for row in rv.rows.values_mut() {
        let mut last_rank = 1u8;
        for (stage, cum_pos) in row.columns.iter_mut().filter_map(get_cum_pos).enumerate() {
            if stage > 0 {
                // show no change in col 1?
                cum_pos.change = last_rank as i8 - cum_pos.pos as i8;
            }
            last_rank = cum_pos.pos;
        }
    }
}

pub fn calc_stage_positions(rv: &mut ResultView) {
    for stage in 0..rv.event.stage_count() {
        let mut positions: Vec<&mut Pos> = vec![];
        for rr in rv.rows.values_mut() {
            if let Some(rs) = &mut rr.columns[stage] {
                positions.push(&mut rs.stage_pos);
            }
        }
        calc_rank(&mut positions);
    }
}

fn calc_rank(positions: &mut [&mut Pos]) {
    // sort by score
    positions.sort_unstable_by_key(|p| p.score_ds);

    // calc the ranks and eq and poke into the Pos
    let mut last_time = 0u16;
    let mut rank = 1u8;
    for (idx, pos) in positions.iter_mut().enumerate() {
        let score = pos.score_ds;
        let eq = score == last_time;
        last_time = score;
        if !eq {
            rank = idx as u8 + 1
        };

        pos.eq = eq;
        pos.pos = rank;
    }
}

pub fn calc_cumulative_positions(rv: &mut ResultView) {
    for stage in 0..rv.event.stage_count() {
        let mut positions: Vec<&mut Pos> = vec![];
        for rr in rv.rows.values_mut() {
            if let Some(rs) = &mut rr.columns[stage] {
                if let Some(cum_pos) = &mut rs.cum_pos {
                    positions.push(cum_pos);
                }
            }
        }
        calc_rank(&mut positions);
    }
}

/// Best-X-of-Y aggregate score for a row, per the event's scoring rule.
/// `best_y <= 1` (the default) means every completed test counts.
fn best_x_of_y(event: &EventInfo, columns: &[Option<ResultScore>]) -> u32 {
    let mut scores: Vec<u32> = columns
        .iter()
        .filter_map(|c| c.as_ref())
        .map(|rs| rs.stage_pos.score_ds as u32)
        .collect();
    scores.sort_unstable();
    let best_y = event.best_y;
    let best_x = event.best_x;
    if best_y <= 1 {
        return scores.iter().sum();
    }
    let consider = best_y.min(scores.len() as u8) as usize;
    let keep = if best_x == 0 || best_x >= best_y {
        consider
    } else {
        (best_x.min(consider as u8)) as usize
    };
    scores[..consider].iter().take(keep).sum()
}

/// Per-row best-X-of-Y total + tie-aware overall rank (1,1,3).
pub fn calc_totals(rv: &mut ResultView) {
    for row in rv.rows.values_mut() {
        row.total_ds = best_x_of_y(&rv.event, &row.columns);
        row.total_pos = 0;
        row.total_eq = false;
    }
    let mut rows: Vec<&mut ResultRow> = rv
        .rows
        .values_mut()
        .filter(|r| r.total_ds != 0) // no completed runs yet
        .collect();
    rows.sort_by_key(|r| r.total_ds);
    let mut last = u32::MAX;
    let mut rank = 1u8;
    for (idx, row) in rows.iter_mut().enumerate() {
        let eq = row.total_ds == last;
        last = row.total_ds;
        if !eq {
            rank = idx as u8 + 1;
        }
        row.total_pos = rank;
        row.total_eq = eq;
    }
}

pub fn calc(rv: &mut ResultView) {
    calc_base_times(rv);
    calc_penalties(rv);
    calc_stage_positions(rv);
    calc_cumulative_times(rv);
    calc_cumulative_positions(rv);
    calc_pos_changes(rv);
    calc_totals(rv);
}

pub fn create_result_view(event: &EventInfo, scores: &[ScoreData], class: &str) -> ResultView {
    // Calc min time per stage (for class)
    // loop raw results... list of cars eligible.  Find relevant results.
    // sort into stages.

    // validate ? Complain about scores for non-existant cars
    // times for non-existant stages

    let mut rv = ResultView::init(class, event, scores);
    calc(&mut rv);
    rv
}

/// The overall standing: every active entry, regardless of class membership.
/// Always available, so the results page can show an Outright tab even when the
/// event's classes list doesn't include one.
pub fn create_outright_view(event: &EventInfo, scores: &[ScoreData]) -> ResultView {
    let mut rv = ResultView::init("Outright", event, scores);
    if !event.classes.iter().any(|c| c == "Outright") {
        rv.rows = event
            .sorted_entries()
            .into_iter()
            .filter(|e| is_active_entry(e))
            .map(|e| (e.entry_no, ResultRow::init(e, event, scores)))
            .collect();
    }
    calc(&mut rv);
    rv
}

// get entries  in class
pub fn find_entries_in_class<'a>(entries: &'a [Entry], class: &str) -> Vec<&'a Entry> {
    let mut v: Vec<&Entry> = entries
        .iter()
        .filter(|e| e.classes.iter().any(|c| c == class) && is_active_entry(e))
        .collect();
    v.sort_by_key(|e| entry_sort_key(e));
    v
}

/// Entries that count in the results: withdrawn / draft / reserve are out.
pub fn is_active_entry(e: &Entry) -> bool {
    !matches!(
        e.status,
        EntryStatus::Withdrawn | EntryStatus::Draft | EntryStatus::Reserve
    )
}

// get available Raw scores for the list of cars in a stage
pub fn find_score<'a>(scores: &'a [ScoreData], car: &str, stage: u8) -> Option<&'a ScoreData> {
    scores.iter().find(|s| s.stage == stage && car == s.car)
}

// ---------------------------------------------------------------------------
// Event id + slug helpers.  The event id is the primary key everywhere:
// localStorage (`event:<id>`), Matrix aliases and timing payloads.
// ---------------------------------------------------------------------------

/// Slugify a string for use in ids/aliases: lowercase `[a-z0-9]` joined by `-`.
pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Extract a plausible 4-digit year from a string, else "".
pub fn year_token(s: &str) -> String {
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 4 {
        return digits[0..4].to_string();
    }
    String::new()
}

/// Build the stable event id from its parts.
///
/// Always `kt-<year>`; the club slug is inserted when the event slug is short
/// (<5 chars) or missing, so abbreviations like `kt-2026-ndc-kcr1` come out
/// naturally.  Both slugs may be user-supplied abbreviations.
pub fn build_event_id(year: &str, club_slug: &str, event_slug: &str) -> String {
    let year = year_token(year);
    let club = slugify(club_slug);
    let ev = slugify(event_slug);
    let mut out = String::from(ID_PREFIX);
    out.push_str(&year);
    let short = ev.is_empty() || ev.len() < 5;
    let push = |out: &mut String, part: &str| {
        if part.is_empty() {
            return;
        }
        if !out.ends_with('-') {
            out.push('-');
        }
        out.push_str(part);
    };
    if short && !club.is_empty() {
        push(&mut out, &club);
    }
    push(&mut out, &ev);
    out
}

/// Validate a candidate event id: `kt-` prefix, contains a 4-digit year,
/// lowercase alnum + `-`, no leading/trailing/double dashes, sane length.
pub fn valid_event_id(id: &str) -> bool {
    if !id.starts_with(ID_PREFIX) {
        return false;
    }
    let body = &id[ID_PREFIX.len()..];
    if body.is_empty() || body.len() > 48 {
        return false;
    }
    if !body
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return false;
    }
    if body.starts_with('-') || body.ends_with('-') || body.contains("--") {
        return false;
    }
    let has_year = body
        .as_bytes()
        .windows(4)
        .any(|w| w.iter().all(|b| b.is_ascii_digit()));
    has_year
}

// ---------------------------------------------------------------------------
// Invite URL.  `?homeserver=..&event=<id>&timing=<alias>&space=<alias>`
// ---------------------------------------------------------------------------

// Invite URL + codec.  Wired to a UI later (timing-unknown / publish flow);
// covered by unit tests today.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub struct Invite {
    pub homeserver: String,
    pub event: String,  // event id
    pub timing: String, // timing room alias, e.g. "#kt-2026-..-timing:localhost"
    pub space: String,  // space room alias
}

#[allow(dead_code)]
fn encode_query(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[allow(dead_code)]
fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[allow(dead_code)]
fn decode_query(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[allow(dead_code)]
impl Invite {
    pub fn to_query(&self) -> String {
        format!(
            "homeserver={}&event={}&timing={}&space={}",
            encode_query(&self.homeserver),
            encode_query(&self.event),
            encode_query(&self.timing),
            encode_query(&self.space),
        )
    }

    pub fn from_query(q: &str) -> Option<Invite> {
        let mut homeserver = None;
        let mut event = None;
        let mut timing = None;
        let mut space = None;
        for pair in q.split('&') {
            let mut it = pair.splitn(2, '=');
            let (k, v) = (it.next()?, it.next()?);
            match k {
                "homeserver" => homeserver = Some(decode_query(v)),
                "event" => event = Some(decode_query(v)),
                "timing" => timing = Some(decode_query(v)),
                "space" => space = Some(decode_query(v)),
                _ => {}
            }
        }
        Some(Invite {
            homeserver: homeserver.unwrap_or_default(),
            event: event?,
            timing: timing.unwrap_or_default(),
            space: space.unwrap_or_default(),
        })
    }

    /// Absolute invite URL for `origin` (e.g. `window.location.origin`).
    pub fn url(&self, origin: &str) -> String {
        format!("{origin}?{}", self.to_query())
    }
}

// ---------------------------------------------------------------------------
// Merge helpers (room history -> local storage)
// ---------------------------------------------------------------------------

/// Insert or overwrite a time for a stage+car in deciseconds.
/// (Wasm-only callers: the sync sink. Kept here for the native build.)
#[allow(dead_code)]
pub fn upsert_ktime(scores: &mut Vec<ScoreData>, stage: u8, car: &str, time: KTime) {
    if let Some(s) = scores.iter_mut().find(|s| s.stage == stage && s.car == car) {
        s.time = time;
    } else {
        scores.push(ScoreData {
            stage,
            car: car.to_string(),
            time,
        });
    }
}

/// Apply an incoming event setup (from the room manifest).  Last-writer-wins./// Accepts when local has no id yet (fresh device) or ids match.
/// Returns true if the local event changed.
#[allow(dead_code)] // wasm-only: used by the sync merge sink
pub fn merge_setup(local: &mut EventInfo, incoming: &EventInfo) -> bool {
    if incoming.id.is_empty() || (!local.id.is_empty() && local.id != incoming.id) {
        return false;
    }
    if local.id.is_empty() {
        local.id = incoming.id.clone();
    }
    let changed = serde_json::to_string(local).ok() != serde_json::to_string(incoming).ok();
    *local = incoming.clone();
    changed
}

const EVENT_SESSION: &str = "event";

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn session_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.session_storage().ok().flatten()
}

/// Load an event's config by replaying its transaction log (last-writer-wins
/// on setup).  Returns the null event (empty id) when the key is empty or the
/// log is empty.
pub fn load_event(key: &str) -> EventInfo {
    if key.is_empty() {
        return EventInfo {
            name: key.to_string(),
            ..Default::default()
        };
    }
    let (mut ev, _, _) =
        crate::replay::replay(&crate::log::load_log(key), &crate::log::load_pending(key));
    ev.ensure_entry_nos();
    ev
}

// ---------------------------------------------------------------------------
// Demo event (local training).  Never published, never joins a room.
// ---------------------------------------------------------------------------

/// Build the pristine demo event template.  Sample entries + stages so
/// officials can practise start/finish timing and watch results compute.
pub fn demo_event() -> EventInfo {
    let mut ev = EventInfo {
        id: DEMO_EVENT_ID.to_string(),
        name: "Khanatime Demo".to_string(),
        sponsoring_club: "Demo Club".to_string(),
        status: EventStatus::Draft,
        stages: (1..=3).map(Stage::for_test).collect(),
        classes: ["Outright", "Female", "Junior"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        ..Default::default()
    };
    for (car, name, classes) in [
        ("1", "Alice", &["Outright", "Female"][..]),
        ("2", "Bob", &["Outright"][..]),
        ("3", "Carol", &["Outright", "Female", "Junior"][..]),
        ("4", "Dan", &["Outright", "Junior"][..]),
        ("5", "Erin", &["Outright", "Female"][..]),
        ("6", "Frank", &["Outright"][..]),
        ("12", "Gail", &["Outright", "Female"][..]),
    ] {
        ev.add_entry(car, name);
        if let Some(entry) = ev.entries.iter_mut().find(|e| e.car == car) {
            entry.classes = classes.iter().map(|s| s.to_string()).collect();
        }
    }
    // Erin and Gail share Erin's MX-5 (a typed shared-car name).
    for car in ["5", "12"] {
        if let Some(entry) = ev.entries.iter_mut().find(|e| e.car == car) {
            entry.shared_car = Some("Erin's MX-5".to_string());
        }
    }
    ev
}

/// Restore the demo event to its pristine template, wiping all training state
/// (entries, stages, times, runs) added while practising.
pub fn reset_demo() {
    crate::log::remove_event_log(DEMO_EVENT_ID);
    ensure_demo();
}

/// Ensure the demo event exists in the transaction log (its setup manifest is
/// the durable record, exactly like any other event's).
pub fn ensure_demo() {
    if crate::log::load_log(DEMO_EVENT_ID).is_empty()
        && crate::log::load_pending(DEMO_EVENT_ID).is_empty()
    {
        enqueue_event_setup(&demo_event());
    }
}

/// Enqueue an event-setup manifest into the event's transaction log (the
/// durable record of its configuration).
pub fn enqueue_event_setup(ev: &EventInfo) {
    if ev.id.is_empty() {
        return;
    }
    let body = format!(
        "{}{}",
        crate::timing_event::TimingEvent::SETUP_PREFIX,
        serde_json::to_string(ev).unwrap()
    );
    crate::log::enqueue_pending(&ev.id, crate::log::LogMsg::new_pending(body, String::new()));
}

// ---------------------------------------------------------------------------
// Publish / amend validation.  Pure, so the rules are unit-testable.
// ---------------------------------------------------------------------------

/// True when the event has any recorded timing data (scores or run records).
pub fn has_timing_data(scores: &[ScoreData], runs: &[RunRecord]) -> bool {
    !scores.is_empty() || !runs.is_empty()
}

/// True when a stage has any recorded score or run record.
pub fn stage_has_timing(scores: &[ScoreData], runs: &[RunRecord], stage: u8) -> bool {
    scores.iter().any(|s| s.stage == stage) || runs.iter().any(|r| r.test == stage)
}

/// True when a car has any recorded score or run record.
pub fn entry_has_timing(scores: &[ScoreData], runs: &[RunRecord], car: &str) -> bool {
    scores.iter().any(|s| s.car == car) || runs.iter().any(|r| r.car == car)
}

/// Reasons the event is not in a publishable state.  Empty when it is.
///
/// A NEW event must publish before any timing happens; an event that already
/// has scores/runs is amended, not republished.
pub fn publish_errors(event: &EventInfo, scores: &[ScoreData], runs: &[RunRecord]) -> Vec<String> {
    let mut errs: Vec<String> = vec![];
    if event.is_demo() {
        errs.push("Demo events are for local training only and can't be published.".to_string());
    }
    if event.stages.is_empty() {
        errs.push("Add at least one test before publishing.".to_string());
    }
    if has_timing_data(scores, runs) {
        errs.push(
            "New events can't be published with timing data — publish before timing starts."
                .to_string(),
        );
    }
    errs
}

/// List of known events (ids) that have a transaction log.
pub fn list_events() -> HashSet<String> {
    crate::log::list_event_ids()
}

// ---------------------------------------------------------------------------
// Run records (start/finish pairing, run numbering, pending starts).
// ---------------------------------------------------------------------------

/// Two records are the same observation (used to dedupe Matrix mirrors).
fn same_run(a: &RunRecord, b: &RunRecord) -> bool {
    a.r#type == b.r#type && a.test == b.test && a.car == b.car && a.run == b.run && a.ts == b.ts
}

/// Append a run, skipping exact duplicates.  Returns true when added.
pub fn add_run(runs: &mut Vec<RunRecord>, run: RunRecord) -> bool {
    if runs.iter().any(|r| same_run(r, &run)) {
        return false;
    }
    runs.push(run);
    true
}

/// The run number the next start for `(test, car)` should use.
pub fn next_run(runs: &[RunRecord], test: u8, car: &str) -> u8 {
    runs.iter()
        .filter(|r| r.test == test && r.car == car)
        .map(|r| r.run)
        .max()
        .map(|m| m + 1)
        .unwrap_or(1)
}

/// Cars that started `test` but have no finish yet, oldest first.
pub fn pending_starts(runs: &[RunRecord], test: u8) -> Vec<&RunRecord> {
    let mut out: Vec<&RunRecord> = runs
        .iter()
        .filter(|r| r.r#type == RUN_START && r.test == test)
        .filter(|r| r.status.as_deref() != Some("dns"))
        .filter(|r| {
            !runs.iter().any(|f| {
                f.r#type == RUN_FINISH && f.test == r.test && f.car == r.car && f.run == r.run
            })
        })
        .collect();
    out.sort_by_key(|r| r.ts);
    out
}

/// Elapsed time between a start and its finish, in deciseconds.
pub fn elapsed_ds(start_ts: i64, finish_ts: i64) -> u16 {
    let diff = finish_ts - start_ts;
    if diff <= 0 {
        0
    } else {
        ((diff + 50) / 100) as u16
    }
}

/// The [KTime] a finish record represents (for the scores / results model).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // wasm sync sink + tests
pub fn finish_to_ktime(r: &RunRecord) -> KTime {
    match r.status.as_deref() {
        Some("dnf") => KTime::DNF,
        Some("fts") => KTime::FTS,
        Some("wd") => KTime::WD,
        Some("nosho") => KTime::NOSHO,
        _ => KTime::Time(KTimeTime {
            time_ds: r.time_ds.unwrap_or(0),
            flags: r.flags.unwrap_or(0),
            garage: r.status.as_deref() == Some("garage"),
        }),
    }
}

/// The [RunRecord] a wire [TimingEvent] represents (used by the sync merge).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // wasm sync sink + tests
pub fn record_from_timing(te: &crate::timing_event::TimingEvent) -> RunRecord {
    RunRecord {
        r#type: te.r#type.clone(),
        test: te.test,
        car: te.car.clone(),
        run: te.run,
        ts: te.ts,
        time_ds: te.time_ds,
        status: te.status.clone(),
        flags: te.flags,
        official_id: te.official_id.clone(),
    }
}

/// Decode an event-setup message body (`khanatime_setup:<json>`).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // wasm sync sink + tests
pub fn from_setup_body(body: &str) -> Option<EventInfo> {
    let json = body.strip_prefix(crate::timing_event::TimingEvent::SETUP_PREFIX)?;
    serde_json::from_str(json).ok()
}

pub fn session_event_name() -> String {
    session_storage()
        .and_then(|st| st.get_item(EVENT_SESSION).ok().flatten())
        .unwrap_or_default()
}

pub fn session_set_event(key: &str) {
    if let Some(st) = session_storage() {
        let _ = st.set_item(EVENT_SESSION, key);
    }
}

pub fn local_role() -> String {
    storage()
        .and_then(|st| st.get_item("kt_role").ok().flatten())
        .unwrap_or_else(|| ROLE_OFFICIAL.to_string())
}

#[allow(dead_code)] // paired with local_role(); a role-picker UI is planned
pub fn set_local_role(role: &str) {
    if let Some(st) = storage() {
        let _ = st.set_item("kt_role", role);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_basics() {
        assert_eq!(slugify("Khanacross Round 1"), "khanacross-round-1");
        assert_eq!(slugify("  --Hello--World! "), "hello-world");
        assert_eq!(slugify("Café 12"), "caf-12");
        assert_eq!(slugify("NDC"), "ndc");
        assert_eq!(slugify("!!!"), "");
    }

    #[test]
    fn year_token_finds_four_digits() {
        assert_eq!(year_token("2026"), "2026");
        assert_eq!(year_token("Round 2026"), "2026");
        assert_eq!(year_token("26"), "");
        assert_eq!(year_token(""), "");
    }

    #[test]
    fn build_event_id_rules() {
        // year only, empty club/event -> bare prefix+year
        assert_eq!(build_event_id("2026", "", ""), "kt-2026");
        // short event slug -> club is inserted (abbreviations work)
        assert_eq!(build_event_id("2026", "NDC", "KCR1"), "kt-2026-ndc-kcr1");
        // full name, no club needed
        assert_eq!(
            build_event_id("2026", "NDC", "Khanacross Round 1"),
            "kt-2026-khanacross-round-1"
        );
        // missing year stays year-less but well-formed
        assert_eq!(build_event_id("", "", "x"), "kt-x");
    }

    #[test]
    fn valid_event_id_checks() {
        assert!(valid_event_id("kt-2026-ndc-kcr1"));
        assert!(valid_event_id("kt-2026"));
        // no year
        assert!(!valid_event_id("kt-aaa"));
        // uppercase
        assert!(!valid_event_id("kt-2026-KCR1"));
        // double dash
        assert!(!valid_event_id("kt-2026--x"));
        // wrong prefix
        assert!(!valid_event_id("2026-ndc"));
    }

    #[test]
    fn invite_round_trip() {
        let inv = Invite {
            homeserver: "http://localhost:8008".to_string(),
            event: "kt-2026-ndc-kcr1".to_string(),
            timing: "#kt-2026-ndc-kcr1-timing:localhost".to_string(),
            space: "#kt-2026-ndc-kcr1:localhost".to_string(),
        };
        let q = inv.to_query();
        assert!(q.contains("timing=%23kt-2026"));
        let back = Invite::from_query(&q).expect("parses");
        assert_eq!(back, inv);
    }

    #[test]
    fn invite_url_builds_absolute() {
        let inv = Invite {
            homeserver: "http://localhost:8008".to_string(),
            event: "kt-2026".to_string(),
            timing: "#t:localhost".to_string(),
            space: "#s:localhost".to_string(),
        };
        assert!(inv
            .url("http://localhost:8080")
            .starts_with("http://localhost:8080?"));
    }

    #[test]
    fn merge_setup_accepts_same_id() {
        let mut local = EventInfo {
            id: "kt-2026-x".into(),
            name: "old".into(),
            ..Default::default()
        };
        let incoming = EventInfo {
            id: "kt-2026-x".into(),
            name: "new".into(),
            sponsoring_club: "NDC".into(),
            ..Default::default()
        };
        assert!(merge_setup(&mut local, &incoming));
        assert_eq!(local.name, "new");
        assert_eq!(local.sponsoring_club, "NDC");
        // idempotent: no change on second merge
        assert!(!merge_setup(&mut local, &incoming));
    }

    #[test]
    fn merge_setup_accepts_empty_local() {
        let mut local = EventInfo::default();
        let incoming = EventInfo {
            id: "kt-2026-x".into(),
            name: "arrived".into(),
            ..Default::default()
        };
        assert!(merge_setup(&mut local, &incoming));
        assert_eq!(local.id, "kt-2026-x");
    }

    #[test]
    fn merge_setup_rejects_other_event() {
        let mut local = EventInfo {
            id: "kt-2026-a".into(),
            ..Default::default()
        };
        let incoming = EventInfo {
            id: "kt-2026-b".into(),
            ..Default::default()
        };
        assert!(!merge_setup(&mut local, &incoming));
        assert_eq!(local.id, "kt-2026-a");
    }

    fn run(r#type: &str, test: u8, car: &str, run: u8, ts: i64) -> RunRecord {
        RunRecord {
            r#type: r#type.into(),
            test,
            car: car.into(),
            run,
            ts,
            ..Default::default()
        }
    }

    #[test]
    fn add_run_dedupes() {
        let mut runs = vec![run("start", 1, "7", 1, 100)];
        assert!(!add_run(&mut runs, run("start", 1, "7", 1, 100)));
        assert!(add_run(&mut runs, run("start", 1, "7", 2, 200)));
        assert_eq!(runs.len(), 2);
    }

    #[test]
    fn next_run_counts_starts() {
        let runs = vec![run("start", 1, "7", 1, 100), run("start", 1, "7", 2, 200)];
        assert_eq!(next_run(&runs, 1, "7"), 3);
        assert_eq!(next_run(&runs, 1, "8"), 1);
        assert_eq!(next_run(&runs, 2, "7"), 1);
    }

    #[test]
    fn pending_starts_hides_finished_and_dns() {
        let mut runs = vec![
            run("start", 1, "7", 1, 100),
            run("start", 1, "8", 1, 200),
            run("start", 1, "9", 1, 300),
            run("finish", 1, "8", 1, 400),
        ];
        let mut dns = run("start", 1, "9", 1, 300);
        dns.status = Some("dns".into());
        runs[2] = dns;
        let pending = pending_starts(&runs, 1);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].car, "7");
        assert!(pending_starts(&runs, 2).is_empty());
    }

    #[test]
    fn elapsed_ds_rounds() {
        assert_eq!(elapsed_ds(0, 0), 0);
        assert_eq!(elapsed_ds(1000, 1000 + 12350), 124); // 12.35s -> 124ds
        assert_eq!(elapsed_ds(1000, 500), 0);
    }

    #[test]
    fn finish_to_ktime_maps_status() {
        let mut c = run("finish", 1, "7", 1, 0);
        c.time_ds = Some(1234);
        c.flags = Some(2);
        assert_eq!(
            finish_to_ktime(&c),
            KTime::Time(KTimeTime {
                time_ds: 1234,
                flags: 2,
                garage: false
            })
        );
        let mut dnf = run("finish", 1, "7", 1, 0);
        dnf.status = Some("dnf".into());
        assert_eq!(finish_to_ktime(&dnf), KTime::DNF);
        let mut g = run("finish", 1, "7", 1, 0);
        g.status = Some("garage".into());
        g.time_ds = Some(55);
        assert_eq!(
            finish_to_ktime(&g),
            KTime::Time(KTimeTime {
                time_ds: 55,
                flags: 0,
                garage: true
            })
        );
    }

    #[test]
    fn record_from_timing_roundtrips() {
        use crate::timing_event::TimingEvent;
        let te = TimingEvent {
            r#type: "finish".into(),
            event_id: "ev".into(),
            test: 2,
            car: "17".into(),
            run: 3,
            ts: 42,
            time_ds: Some(999),
            status: Some("clean".into()),
            flags: Some(1),
            official_id: Some("u".into()),
        };
        let r = record_from_timing(&te);
        assert_eq!(r.r#type, "finish");
        assert_eq!(r.test, 2);
        assert_eq!(r.car, "17");
        assert_eq!(r.run, 3);
        assert_eq!(r.ts, 42);
        assert_eq!(r.time_ds, Some(999));
        assert_eq!(r.status.as_deref(), Some("clean"));
        assert_eq!(r.flags, Some(1));
        assert_eq!(r.official_id.as_deref(), Some("u"));
    }

    #[test]
    fn from_setup_body_decodes_event() {
        let ev = EventInfo {
            id: "kt-2026-x".into(),
            name: "Demo".into(),
            sponsoring_club: "NDC".into(),
            ..Default::default()
        };
        let body = format!(
            "{}{}",
            crate::timing_event::TimingEvent::SETUP_PREFIX,
            serde_json::to_string(&ev).unwrap()
        );
        let decoded = from_setup_body(&body).expect("setup body decodes");
        assert_eq!(decoded.id, ev.id);
        assert_eq!(decoded.name, ev.name);
        assert!(from_setup_body("khanatime_result:{}").is_none());
    }

    #[test]
    fn demo_event_is_local_only() {
        let demo = demo_event();
        assert!(demo.is_demo());
        assert!(!valid_event_id(&demo.id), "demo id must not be publishable");
        assert_eq!(demo.name, "Khanatime Demo");
        assert_eq!(demo.stages.len(), 3);
        assert!(!demo.entries.is_empty());
        assert!(!demo.classes.is_empty());
        // A demo event carries no timing data.
        assert!(!has_timing_data(&[], &[]));
    }

    #[test]
    fn publish_errors_catch_bad_state() {
        let mut ev = EventInfo {
            id: "kt-2026-x".into(),
            name: "X".into(),
            stages: vec![],
            ..Default::default()
        };
        let score = ScoreData {
            stage: 1,
            car: "1".into(),
            time: KTime::Time(KTimeTime {
                time_ds: 100,
                flags: 0,
                garage: false,
            }),
        };
        let run = RunRecord {
            r#type: RUN_START.to_string(),
            test: 1,
            car: "1".into(),
            run: 1,
            ts: 1,
            ..Default::default()
        };
        // No stages -> error.
        assert!(!publish_errors(&ev, &[], &[]).is_empty());
        // Stages configured -> clean.
        ev.stages = vec![Stage::for_test(1)];
        assert!(publish_errors(&ev, &[], &[]).is_empty());
        // Timing data -> error.
        assert!(publish_errors(&ev, &[score], &[]).contains(
            &"New events can't be published with timing data — publish before timing starts."
                .to_string()
        ));
        assert!(publish_errors(&ev, &[], &[run]).contains(
            &"New events can't be published with timing data — publish before timing starts."
                .to_string()
        ));
        // Demo -> error.
        let demo = demo_event();
        assert!(publish_errors(&demo, &[], &[])
            .iter()
            .any(|e| e.contains("can't be published")));
    }

    #[test]
    fn stage_and_entry_timing_guards() {
        let scores = vec![ScoreData {
            stage: 2,
            car: "7".into(),
            time: KTime::Time(KTimeTime {
                time_ds: 100,
                flags: 0,
                garage: false,
            }),
        }];
        let runs = vec![RunRecord {
            r#type: RUN_FINISH.to_string(),
            test: 3,
            car: "8".into(),
            run: 1,
            ts: 1,
            ..Default::default()
        }];
        assert!(stage_has_timing(&scores, &runs, 2));
        assert!(stage_has_timing(&scores, &runs, 3));
        assert!(!stage_has_timing(&scores, &runs, 1));
        assert!(entry_has_timing(&scores, &runs, "7"));
        assert!(entry_has_timing(&scores, &runs, "8"));
        assert!(!entry_has_timing(&scores, &runs, "9"));
    }

    #[test]
    fn results_exclude_withdrawn_entries() {
        let mut e1 = Entry::new("1", "Alice");
        e1.status = EntryStatus::Started;
        let mut e2 = Entry::new("2", "Bob");
        e2.status = EntryStatus::Withdrawn;
        let mut e3 = Entry::new("3", "Carol");
        e3.status = EntryStatus::Reserve;
        let entries = vec![e1, e2, e3];
        let found = find_entries_in_class(&entries, "Outright");
        let cars: Vec<&str> = found.iter().map(|e| e.car.as_str()).collect();
        assert_eq!(cars, vec!["1"]);
    }

    #[test]
    fn outright_view_includes_every_active_entry() {
        // Entry without the "Outright" tag must still be in the overall view.
        let mut ev = EventInfo {
            stages: vec![],
            classes: vec!["Female".into()],
            entries: vec![],
            ..Default::default()
        };
        let mut a = Entry::new("1", "Alice");
        a.entry_no = 1;
        a.classes = vec!["Female".into()];
        let mut b = Entry::new("2", "Bob");
        b.entry_no = 2;
        b.classes = vec!["Junior".into()];
        let mut w = Entry::new("3", "Wendy");
        w.entry_no = 3;
        w.status = EntryStatus::Withdrawn;
        ev.entries = vec![a, b, w];
        let rv = create_outright_view(&ev, &[]);
        let nos: Vec<u32> = rv.rows.keys().copied().collect();
        assert_eq!(nos, vec![1, 2]);
    }

    #[test]
    fn best_x_of_y_rules() {
        let mut ev = EventInfo {
            stages_count: 3,
            best_x: 1,
            best_y: 1, // default: all count
            ..Default::default()
        };
        let cols = |ds: &[u32]| -> Vec<Option<ResultScore>> {
            ds.iter()
                .map(|d| {
                    Some(ResultScore {
                        stage_pos: Pos::init(*d as u16),
                        ..Default::default()
                    })
                })
                .collect()
        };
        let row = cols(&[100, 200, 300]);
        assert_eq!(best_x_of_y(&ev, &row), 600);
        ev.best_x = 1;
        ev.best_y = 2;
        assert_eq!(best_x_of_y(&ev, &row), 100); // best 1 of first 2
        ev.best_x = 2;
        ev.best_y = 2;
        assert_eq!(best_x_of_y(&ev, &row), 300); // best 2 of first 2
    }

    #[test]
    fn default_event_has_stages() {
        let ev = EventInfo::default();
        assert_eq!(ev.stage_count(), 3);
        let first = ev.stage(0);
        assert_eq!(first.num, 1);
        assert_eq!(first.name, "Test 1");
        assert_eq!(first.repeats, 1);
        assert_eq!(first.best_x, 1);
        assert_eq!(first.timing, TimingStyle::Stopwatch);
    }

    #[test]
    fn legacy_event_migrates_to_stages() {
        // A pre-per-stage event: only stages_count / best_x / best_y set.
        let json = r#"{
            "name": "Legacy",
            "stages_count": 3,
            "classes": ["Outright"],
            "entries": [],
            "best_x": 1,
            "best_y": 2
        }"#;
        let mut ev: EventInfo = serde_json::from_str(json).unwrap();
        ev.ensure_stages();
        assert_eq!(ev.stage_count(), 3);
        let s = ev.stage(0);
        assert_eq!(s.num, 1);
        assert_eq!(s.repeats, 2);
        assert_eq!(s.best_x, 1);
    }

    #[test]
    fn stages_roundtrip_via_json() {
        let ev = EventInfo {
            stages: vec![
                Stage::for_test(1),
                Stage {
                    num: 2,
                    name: "Creek".into(),
                    repeats: 3,
                    best_x: 2,
                    timing: TimingStyle::Rally,
                },
            ],
            ..Default::default()
        };
        let json = serde_json::to_string(&ev).unwrap();
        let back: EventInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.stage_count(), 2);
        assert_eq!(back.stage(1).name, "Creek");
        assert_eq!(back.stage(1).repeats, 3);
        assert_eq!(back.stage(1).timing, TimingStyle::Rally);
    }

    #[test]
    fn entry_default_status_is_submitted() {
        let e = Entry::new("7", "Alice");
        assert_eq!(e.status, EntryStatus::Submitted);
        assert_eq!(e.status.to_string(), "entry submitted");
        assert_eq!(e.status.as_str(), "submitted");
    }

    #[test]
    fn entry_status_roundtrip_via_json() {
        for (s, value) in [
            (EntryStatus::Draft, "draft"),
            (EntryStatus::Submitted, "submitted"),
            (EntryStatus::Accepted, "accepted"),
            (EntryStatus::Reserve, "reserve"),
            (EntryStatus::Confirmed, "confirmed"),
            (EntryStatus::Started, "started"),
            (EntryStatus::Withdrawn, "withdrawn"),
        ] {
            let e = Entry {
                car: "7".into(),
                name: "Alice".into(),
                status: s.clone(),
                ..Default::default()
            };
            let json = serde_json::to_string(&e).unwrap();
            assert!(json.contains(&format!("\"status\":\"{value}\"")), "{json}");
            let back: Entry = serde_json::from_str(&json).unwrap();
            assert_eq!(back.status, s);
        }

        // Legacy entries (no status field) read as the default (submitted),
        // and the old "entered" value is still accepted.
        let legacy = r#"{"car":"8","name":"Bob","classes":["Outright"]}"#;
        let back: Entry = serde_json::from_str(legacy).unwrap();
        assert_eq!(back.status, EntryStatus::Submitted);
        let old = r#"{"car":"9","name":"Carol","status":"entered"}"#;
        let back: Entry = serde_json::from_str(old).unwrap();
        assert_eq!(back.status, EntryStatus::Submitted);
    }

    #[test]
    fn set_entry_status_updates_by_entry_no() {
        let mut ev = EventInfo::default();
        ev.add_entry("7", "Alice");
        ev.add_entry("8", "Bob");
        let (a, b) = (ev.entries[0].entry_no, ev.entries[1].entry_no);
        assert!(ev.set_entry_status(a, EntryStatus::Accepted));
        assert!(ev.set_entry_status(b, EntryStatus::Withdrawn));
        assert!(!ev.set_entry_status(99, EntryStatus::Draft));
        assert_eq!(ev.find_entry(a).unwrap().status, EntryStatus::Accepted);
        assert_eq!(ev.find_entry(b).unwrap().status, EntryStatus::Withdrawn);
    }

    #[test]
    fn calc_pipeline_handles_multi_stage() {
        let mut a = Entry::new("1", "Alice");
        a.entry_no = 1;
        let mut b = Entry::new("2", "Bob");
        b.entry_no = 2;
        let ev = EventInfo {
            entries: vec![a, b],
            ..Default::default()
        };
        let scores = vec![
            ScoreData {
                stage: 1,
                car: "1".into(),
                time: KTime::Time(KTimeTime {
                    time_ds: 450,
                    flags: 0,
                    garage: false,
                }),
            },
            ScoreData {
                stage: 2,
                car: "1".into(),
                time: KTime::Time(KTimeTime {
                    time_ds: 470,
                    flags: 0,
                    garage: false,
                }),
            },
            ScoreData {
                stage: 1,
                car: "2".into(),
                time: KTime::Time(KTimeTime {
                    time_ds: 500,
                    flags: 0,
                    garage: false,
                }),
            },
            ScoreData {
                stage: 2,
                car: "2".into(),
                time: KTime::Time(KTimeTime {
                    time_ds: 520,
                    flags: 0,
                    garage: false,
                }),
            },
        ];
        let rv = create_result_view(&ev, &scores, "Outright");
        assert_eq!(
            rv.rows.keys().cloned().collect::<Vec<_>>(),
            vec![1u32, 2u32],
            "entries={:?} classes={:?}",
            ev.entries
                .iter()
                .map(|e| (&e.car, &e.classes))
                .collect::<Vec<_>>(),
            ev.classes
        );
        let alice = &rv.rows[&1u32].columns;
        assert_eq!(alice.len(), 3);
        // cumulative time after stage 2 = sum of both stage scores
        assert_eq!(
            alice[1]
                .as_ref()
                .unwrap()
                .cum_pos
                .as_ref()
                .unwrap()
                .score_ds,
            920
        );
    }

    #[test]
    fn entry_msg_roundtrip() {
        let mut entry = Entry::new("7", "Alice");
        entry.owner = Some("@alice:localhost".into());
        entry.status = EntryStatus::Submitted;
        let body = entry_body("kt-2026-x", &entry, false);
        let parsed = from_entry_body(&body).expect("decodes");
        assert_eq!(parsed.event_id, "kt-2026-x");
        assert_eq!(parsed.entry, entry);
        assert!(!parsed.delete);
        assert!(parsed.ts > 0);
        assert!(from_entry_body("khanatime_setup:{}").is_none());
        assert!(from_entry_body("KT {}").is_none());
    }

    #[test]
    fn entry_msg_tombstone() {
        let entry = Entry::new("9", "Dan");
        let body = entry_body("kt-2026-x", &entry, true);
        let parsed = from_entry_body(&body).expect("decodes");
        assert!(parsed.delete);
    }

    #[test]
    fn upsert_entry_replaces_by_entry_no() {
        let mut ev = EventInfo {
            id: "kt-2026-x".into(),
            ..Default::default()
        };
        ev.add_entry("7", "Alice");
        let no = ev.entries[0].entry_no;
        let mut changed = Entry::new("7", "Alice");
        changed.entry_no = no;
        changed.status = EntryStatus::Confirmed;
        changed.owner = Some("@alice:localhost".into());
        assert!(!ev.upsert_entry(changed.clone()));
        assert_eq!(ev.entries.len(), 1);
        assert_eq!(ev.entries[0].status, EntryStatus::Confirmed);
        assert_eq!(ev.entries[0].owner.as_deref(), Some("@alice:localhost"));
        assert!(ev.upsert_entry(Entry::new("8", "Bob")));
        assert_eq!(ev.entries.len(), 2);
    }

    #[test]
    fn upsert_entry_assigns_numbers_and_survives_collisions() {
        let mut ev = EventInfo::default();
        ev.add_entry("7", "Alice"); // entry_no 1
                                    // New entry with no number assigned yet -> gets the next number.
        assert!(ev.upsert_entry(Entry::new("8", "Bob")));
        assert_eq!(ev.entries[1].entry_no, 2);
        // Concurrent offline creation on another device grabbed the same
        // counter for a different owner: renumber, don't clobber.
        let mut incoming = Entry::new("9", "Carol");
        incoming.entry_no = 2;
        incoming.owner = Some("@carol:localhost".into());
        ev.entries[1].owner = Some("@bob:localhost".into());
        assert!(ev.upsert_entry(incoming));
        assert_eq!(ev.entries.len(), 3);
        assert_eq!(ev.entries[2].entry_no, 3);
        assert!(ev.find_entry_by_car("8").is_some());
        // Same owner re-sending is a normal replace.
        let mut resend = Entry::new("9", "Carol");
        resend.entry_no = 3;
        resend.owner = Some("@carol:localhost".into());
        assert!(!ev.upsert_entry(resend));
        assert_eq!(ev.entries.len(), 3);
    }

    #[test]
    fn sorted_entries_orders_explicit_then_arrival() {
        let mut ev = EventInfo::default();
        ev.add_entry("1", "Alice");
        ev.add_entry("2", "Bob");
        ev.add_entry("3", "Carol");
        // No explicit order -> arrival order.
        let names: Vec<&str> = ev
            .sorted_entries()
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(names, vec!["Alice", "Bob", "Carol"]);
        // Explicit order wins; unset entries fall back to entry_no at the end.
        ev.entries[2].order = 10; // Carol first
        ev.entries[0].order = 20; // Alice second
        let names: Vec<&str> = ev
            .sorted_entries()
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(names, vec!["Carol", "Alice", "Bob"]);
    }

    #[test]
    fn entry_default_owner_is_none() {
        let entry = Entry::new("7", "Alice");
        assert_eq!(entry.owner, None);
    }

    #[test]
    fn normalize_car_number_rules() {
        // Whitespace stripped, uppercased.
        assert_eq!(normalize_car_number(" 007 ").unwrap(), "007");
        assert_eq!(normalize_car_number("00a").unwrap(), "00A");
        assert_eq!(normalize_car_number("0 07").unwrap(), "007");
        assert_eq!(normalize_car_number("24TBC").unwrap(), "24TBC");
        assert_eq!(normalize_car_number("7x").unwrap(), "7X");
        // Invalid: empty, letters first, digits after letters, junk, too long.
        assert!(normalize_car_number("").is_err());
        assert!(normalize_car_number("  ").is_err());
        assert!(normalize_car_number("A1").is_err());
        assert!(normalize_car_number("7A2").is_err());
        assert!(normalize_car_number("7-8").is_err());
        assert!(normalize_car_number("1234567890").is_err());
        assert!(validate_car_number("7A").is_ok());
    }

    #[test]
    fn next_free_number_skips_numeric_equivalents() {
        let mut used: HashSet<String> = ["1", "2", "007", "7X"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(next_free_number(&used), "3");
        used.insert("3".into());
        assert_eq!(next_free_number(&used), "4");
        assert_eq!(next_free_number(&HashSet::new()), "1");
    }

    #[test]
    fn suggest_car_number_prefers_when_free() {
        let used: HashSet<String> = ["1", "2", "7A"].iter().map(|s| s.to_string()).collect();
        // Preferred number free -> verbatim.
        assert_eq!(suggest_car_number(&used, "42"), "42");
        // Preferred taken -> next free.
        assert_eq!(suggest_car_number(&used, "2"), "3");
        // Invalid or blank preferred -> next free.
        assert_eq!(suggest_car_number(&used, ""), "3");
        assert_eq!(suggest_car_number(&used, "A9"), "3");
        // Case/space-normalised preference is respected by callers; here raw.
        assert_eq!(suggest_car_number(&used, "7a"), "3");
    }

    #[test]
    fn shared_groups_require_two_members() {
        let mut e1 = Entry::new("1", "Alice");
        e1.entry_no = 1;
        e1.shared_car = Some("ABC123".to_string());
        let mut e2 = Entry::new("2", "Bob");
        e2.entry_no = 2;
        e2.shared_car = Some("abc123".to_string()); // same as e1 after case normalisation
        let mut e3 = Entry::new("3", "Carol");
        e3.entry_no = 3;
        e3.shared_car = Some("Own car".to_string());
        let entries = vec![e1, e2.clone(), e3.clone()];
        let groups = shared_groups(&entries);
        assert_eq!(groups.len(), 1);
        let (name, members) = &groups[0];
        assert_eq!(name, "ABC123"); // first-seen casing
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].name, "Alice");
        assert_eq!(members[1].name, "Bob");
        // Singleton groups don't count as shared.
        let solo = vec![e2, e3];
        assert!(shared_groups(&solo).is_empty());
        // Member entry numbers are flagged.
        let shared = shared_entry_nos(&entries);
        assert!(shared.contains(&1) && shared.contains(&2) && !shared.contains(&3));
    }

    #[test]
    fn ensure_entry_nos_backfills_zeroes() {
        let mut ev = EventInfo::default();
        let mut a = Entry::new("7", "Alice");
        let mut b = Entry::new("8", "Bob");
        b.entry_no = 5;
        ev.entries = vec![a.clone(), b.clone()];
        ev.ensure_entry_nos();
        assert_eq!(ev.find_entry_by_car("7").unwrap().entry_no, 6);
        assert_eq!(ev.find_entry_by_car("8").unwrap().entry_no, 5);
        // Idempotent.
        ev.ensure_entry_nos();
        assert_eq!(ev.find_entry_by_car("7").unwrap().entry_no, 6);
        a.entry_no = 0;
    }

    #[test]
    fn demo_event_has_shared_pair() {
        let demo = demo_event();
        let groups = shared_groups(&demo.entries);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].1.len(), 2);
    }

    #[test]
    fn normalize_car_number_edge_cases() {
        // Internal whitespace stripped.
        assert_eq!(normalize_car_number(" 0 07 ").unwrap(), "007");
        assert_eq!(normalize_car_number("0\t07").unwrap(), "007");
        assert_eq!(normalize_car_number("0\n07").unwrap(), "007");
        // Whitespace only → empty → error.
        assert!(normalize_car_number("  ").is_err());
        assert!(normalize_car_number("\t\n").is_err());
        // Leading zeros preserved.
        assert_eq!(normalize_car_number("007").unwrap(), "007");
        assert_eq!(normalize_car_number("000").unwrap(), "000");
        // Length boundary.
        assert_eq!(normalize_car_number("12345678").unwrap(), "12345678");
        assert!(normalize_car_number("123456789").is_err());
        assert!(
            normalize_car_number("1234567A").is_ok(),
            "digits then letter, 8 chars: valid"
        );
    }

    #[test]
    fn validate_car_number_direct() {
        assert!(validate_car_number("0").is_ok());
        assert!(validate_car_number("7A").is_ok());
        assert!(validate_car_number("24TBC").is_ok());
        assert!(validate_car_number("12X").is_ok());
        // Starts with letter.
        assert!(validate_car_number("A1").is_err());
        // Digits after letters.
        assert!(validate_car_number("7A2").is_err());
        // Too long.
        assert!(validate_car_number("123456789").is_err());
        // Empty.
        assert!(validate_car_number("").is_err());
    }

    #[test]
    fn suggest_car_number_numeric_equivalence_input() {
        let mut used: HashSet<String> = ["007", "7X"].iter().map(|s| s.to_string()).collect();
        // "7" is string-wise free even though "007" is taken.
        assert_eq!(suggest_car_number(&used, "7"), "7");
        used.insert("7".into());
        assert_eq!(suggest_car_number(&used, "7"), "1");
        // "007" taken → preferred "8" free → verbatim.
        assert_eq!(suggest_car_number(&used, "8"), "8");
    }

    #[test]
    fn suggest_car_number_exhaustion_does_not_loop() {
        // Block numbers 1..=100 so next_free must go beyond or hit the
        // MAX_SUGGESTED_NUMBER guard.  We can't exhaust the full pool in a
        // test, but we can verify the bounded loop doesn't hang.
        let mut used: HashSet<String> = (1..=100u32).map(|n| n.to_string()).collect();
        used.insert("101".into());
        assert_eq!(suggest_car_number(&used, ""), "102");
    }

    #[test]
    fn entry_diff_car_assignment_and_clearing() {
        use crate::batch::{entry_diff, EditOp};
        let current = vec![
            // Entry without assigned number.
            Entry {
                entry_no: 1,
                car: String::new(),
                name: "Alice".into(),
                ..Entry::new("", "")
            },
        ];
        // Assign "77".
        let mut assigned = current[0].clone();
        assigned.car = "77".into();
        let lines = entry_diff(&[EditOp::Upsert(assigned.clone())], &current);
        assert!(lines.join("\n").contains("number: (unassigned) → 77"));

        // Clear "77" back to "".
        let cleared = current[0].clone();
        let lines = entry_diff(&[EditOp::Upsert(cleared.clone())], &[assigned]);
        assert!(lines.join("\n").contains("number: 77 → (unassigned)"));
    }

    #[test]
    fn shared_groups_edge_cases() {
        // Withdrawn entries with shared names are still grouped (visible for
        // historical reference).
        let mut e1 = Entry::new("1", "Alice");
        e1.entry_no = 1;
        e1.shared_car = Some("Team Car".into());
        let mut e2 = Entry::new("2", "Bob");
        e2.entry_no = 2;
        e2.shared_car = Some("Team Car".into());
        e2.status = EntryStatus::Withdrawn;
        let items = vec![e1, e2];
        let groups = shared_groups(&items);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].1.len(), 2);

        // Whitespace-only name is treated as None (skipped).
        let mut e3 = Entry::new("3", "Carol");
        e3.entry_no = 3;
        e3.shared_car = Some("   ".into());
        assert!(shared_groups(&[e3]).is_empty());

        // All None → empty groups.
        let e4 = Entry::new("4", "Dan");
        assert!(shared_groups(&[e4]).is_empty());
    }

    #[test]
    fn close_entries_integration() {
        use std::collections::HashSet;

        // Simulate entries awaiting close-entries:
        //   Alice: active, no preferred, no order
        //   Bob: active, preferred 7, no order
        //   Carol: draft (no number needed)
        let mut alice = Entry::new("", "Alice");
        alice.entry_no = 1;
        alice.status = EntryStatus::Submitted;
        let mut bob = Entry::new("", "Bob");
        bob.entry_no = 2;
        bob.preferred_car = "7".into();
        bob.status = EntryStatus::Submitted;
        let mut carol = Entry::new("", "Carol");
        carol.entry_no = 3;
        carol.status = EntryStatus::Draft; // not active

        let entries = vec![alice, bob, carol];
        let mut sorted = entries.clone();
        sorted.sort_by_key(entry_sort_key);

        let committed_cars: HashSet<String> = entries
            .iter()
            .map(|e| e.car.clone())
            .filter(|c| !c.is_empty())
            .collect();
        let mut used = committed_cars;
        let mut staged: Vec<Entry> = vec![];

        for (idx, e) in sorted.iter_mut().enumerate() {
            let active = matches!(
                e.status,
                EntryStatus::Submitted
                    | EntryStatus::Accepted
                    | EntryStatus::Confirmed
                    | EntryStatus::Started
            );
            if !active {
                continue;
            }
            let mut changed = false;
            if e.order == 0 {
                e.order = (idx as u32 + 1) * 10;
                changed = true;
            }
            if e.car.is_empty() {
                let suggest = suggest_car_number(&used, &e.preferred_car);
                e.car = suggest.clone();
                used.insert(suggest);
                changed = true;
            }
            if changed {
                staged.push(e.clone());
            }
        }

        assert_eq!(
            staged.len(),
            2,
            "Alice + Bob get numbers, Carol draft skipped"
        );
        // Alice has no preferred → next free number (1, since "1" not used).
        let alice_result = staged.iter().find(|e| e.entry_no == 1).unwrap();
        assert_eq!(alice_result.car, "1");
        assert_eq!(alice_result.order, 10);
        // Bob preferred "7" and "7" is free → gets it.
        let bob_result = staged.iter().find(|e| e.entry_no == 2).unwrap();
        assert_eq!(bob_result.car, "7");
        assert_eq!(bob_result.order, 20);
        // Carol unchanged.
        assert!(staged.iter().all(|e| e.entry_no != 3));
    }

    #[test]
    fn shared_car_key_normalises() {
        assert_eq!(shared_car_key("ABC123"), "abc123");
        assert_eq!(shared_car_key(" abc 123 "), "abc 123");
        assert_eq!(shared_car_key("Bob's MX5"), "bob's mx5");
        assert_eq!(shared_car_key("Erin's   MX-5"), "erin's mx-5");
    }
}
