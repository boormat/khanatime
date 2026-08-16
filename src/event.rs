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
pub const RUN_STOP: &str = "stop";
pub const RUN_FINISH: &str = "finish";

/// One start, stop, or finish observation for a car on a test.  Persisted per event
/// under `runs:<id>` and exchanged over Matrix as a [TimingEvent].
#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct RunRecord {
    /// Observation id — the indelible thing.  Generated at enqueue time;
    /// carried on the wire so mirrors across transports collapse to one.
    pub uid: String,
    pub r#type: String, // RUN_START | RUN_STOP | RUN_FINISH
    pub test: u8,
    pub car: String,
    pub ts: i64, // ms since epoch
    #[serde(default)]
    pub time_ds: Option<u16>,
    #[serde(default)]
    pub status: Option<String>, // "clean"|"dnf"|"fts"|"wd"|"garage"|"nosho"
    #[serde(default)]
    pub flags: Option<u8>,
    #[serde(default)]
    pub official_id: Option<String>,
    #[serde(default)]
    pub comment: Option<String>,
    /// UIDs of the start/stop observations this finish references (audit trail).
    #[serde(default)]
    pub refs: Vec<String>,
    /// Derived (replay) state: the observation was `void`ed.  Never on the wire.
    #[serde(skip)]
    pub voided: bool,
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
/// `num` gives the stage its display/ordering number.  `runs_total` is the
/// total number of runs each car attempts and `runs_scored` is how many of
/// those count towards the stage score (best N of total).  These are captured
/// on the setup page; the results engine sums the car's best `runs_scored`
/// runs per test.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Stage {
    pub num: u8, // display/ordering number, e.g. 1..12
    #[serde(default)]
    pub name: String,
    /// Total runs per car per test.
    #[serde(default = "default_one", rename = "repeats")]
    pub runs_total: u8,
    /// Scored runs (best N of total; N <= runs_total).
    #[serde(default = "default_one", rename = "best_x")]
    pub runs_scored: u8,
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
    pub fn new(name: String, runs_total: u8, runs_scored: u8, timing: TimingStyle) -> Self {
        Self {
            num: 1,
            name,
            runs_total,
            runs_scored,
            timing,
        }
    }

    /// A default stage for test `num`, as used when seeding a fresh event.
    pub fn for_test(num: u8) -> Self {
        let name = format!("Test {num}");
        Self {
            num,
            name,
            runs_total: 1,
            runs_scored: 1,
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

    // scores: HashMap<i8, HashMap<String, CalcScore>>, // calculated for display.  Key is [stage][car] holding a Score.
    pub classes: Vec<String>, // list of known classes. Order as per display
    pub entries: Vec<Entry>,  // list of know entrants/drivers. Ordered by something

    // ---- draft / publish fields (set up front, editable later) ----
    /// Stable wire identity. Generated once at draft creation; never renames.
    /// The human slug (`id`) is the storage/alias key; `uid` is what timing
    /// and entry messages carry.
    pub uid: String,
    /// Stable primary key. Generated once at draft creation; renames never change it.
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
    pub status: EventStatus,

    // ---- Matrix (populated on publish) ----
    /// Homeserver the event is published on.  Required before publish; drives
    /// which session is resumed and which homeserver the invite points at.
    #[serde(default)]
    pub homeserver: String,
    /// How a scanned invite should sign in on that homeserver (`open` =
    /// auto-register an account; `sso` = public HS, offer SSO only).
    #[serde(default)]
    pub reg: RegistrationMode,
    #[serde(default)]
    pub space_id: Option<String>,
    #[serde(default)]
    pub space_alias: Option<String>,
    #[serde(default)]
    pub timing_id: Option<String>,
    #[serde(default)]
    pub timing_alias: Option<String>,

    // ---- optional event config (editable anytime) ----
    /// Optional parent room (club/organisation space) this event links into.
    #[serde(default)]
    pub parent_room: String,
    /// Allow competitors to self-enter in the app (the Entries page form).
    /// Off by default; officials can always manage entries.
    #[serde(default)]
    pub entries_enabled: bool,
    /// Optional Element Web origin for opening the event's rooms (auto-defaults
    /// per homeserver: app.element.io for Matrix, localhost:8085 for a custom
    /// homeserver).
    #[serde(default)]
    pub element_link: String,
}

/// How a scanned invite should authenticate on its homeserver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RegistrationMode {
    /// Public homeserver (e.g. matrix.org): never auto-register, offer SSO.
    #[default]
    Sso,
    /// Event/local homeserver with open registration: auto-register a fresh
    /// account if the device has no stored session for it.
    Open,
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
    /// The car's runs for this test, in run order, with counting flags set
    /// (the non-counting ones are struck out in the results view).
    pub runs: Vec<RunScore>,
    /// Result within stage; None until the test is completed (enough runs).
    pub stage_pos: Option<Pos>,
    pub cum_pos: Option<Pos>, // pos in event.
}

/// One run of a car in a test, as shown in the results Time cell.
#[derive(Default, Clone, Debug)]
pub struct RunScore {
    /// Timestamp of the original observation (used for ordering and tiebreaking).
    pub ts: i64,
    /// As-entered result (a clean time with flags/garage, or DNF/FTS/WD/DNS).
    pub time: KTime,
    /// Net score (elapsed + penalties, or the aborted/DNS base-penalty).
    pub score: u32,
    /// True when this run is one of the counting best-X (or the kept fallback
    /// when the car has no clean time).
    pub counted: bool,
}

/// Result of the best-X-of-Y aggregation for one car in one test.
#[derive(Default, Debug)]
pub struct StageScore {
    /// Aggregate score, or None until the entrant has completed enough runs
    /// (a cancelled stage, Y = 0, never has counting runs).
    pub sum: Option<u32>,
    pub runs: Vec<RunScore>,
}

//////////////////////////////////////////////////////////////////////
/// impl time
impl Default for EventInfo {
    fn default() -> Self {
        let classes = ["Outright", "Female", "Junior"];
        let classes = classes.map(String::from).into();
        let name = "".into();
        let stages: Vec<Stage> = (1..=1).map(Stage::for_test).collect();
        let entries = vec![];
        Self {
            name,
            stages,
            classes,
            entries,
            uid: String::new(),
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
            status: EventStatus::Draft,
            homeserver: String::new(),
            reg: RegistrationMode::default(),
            space_id: None,
            space_alias: None,
            timing_id: None,
            timing_alias: None,
            parent_room: String::new(),
            entries_enabled: false,
            element_link: String::new(),
        }
    }
}

impl EventInfo {
    /// True when no event is currently selected (the null event).
    pub fn is_null(&self) -> bool {
        self.id.is_empty()
    }

    /// Fill the wire identity with a fresh generated id when it's empty.
    pub fn ensure_uid(&mut self) {
        if self.uid.is_empty() {
            self.uid = crate::ids::gen_short_id();
        }
    }

    /// True for the local training event.  Demo events are never published and
    /// never join a timing room (see [demo_event]).
    pub fn is_demo(&self) -> bool {
        self.id.starts_with("demo-")
    }

    /// True when the event was published to a Matrix room (has the room ids +
    /// homeserver needed to re-join it).
    pub fn is_published(&self) -> bool {
        !self.homeserver.is_empty() && self.space_id.is_some() && self.timing_id.is_some()
    }

    /// The invite a published event joins by — enough to connect to its
    /// homeserver and adopt the event by room id, exactly like a scanned link.
    pub fn invite(&self) -> Option<Invite> {
        if !self.is_published() {
            return None;
        }
        Some(Invite {
            homeserver: self.homeserver.clone(),
            event: self.id.clone(),
            sid: self.space_id.clone().unwrap_or_default(),
            tid: self.timing_id.clone().unwrap_or_default(),
            reg: self.reg,
        })
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

impl ResultView {
    pub fn init(class: &str, event: &EventInfo, runs: &[RunRecord]) -> Self {
        let entries = find_entries_in_class(&event.entries, class);

        let base_times_ds = base_times_for(event, runs);

        let rows: IndexMap<u32, ResultRow> = entries
            .iter()
            .map(|e| (e.entry_no, ResultRow::init(e, event, runs, &base_times_ds)))
            .collect();
        let class = class.to_string();

        Self {
            class,
            event: event.clone(),
            rows,
            base_times_ds,
        }
    }
}

impl ResultRow {
    pub fn init(entry: &Entry, event: &EventInfo, runs: &[RunRecord], base_times: &[u16]) -> Self {
        let columns = (0..event.stage_count())
            .map(|i| {
                let stage = event.stage(i);
                stage_result(&stage, runs, i as u8 + 1, &entry.car, base_times[i]).map(|ss| {
                    ResultScore {
                        stage_pos: ss.sum.map(|s| Pos::init(s as u16)),
                        cum_pos: None,
                        runs: ss.runs,
                    }
                })
            })
            .collect();

        Self {
            entry: entry.clone(),
            columns,
        }
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

/// Net score for a single completed run: elapsed deciseconds plus a 5s penalty
/// per flag / garage return.  Aborted runs score a flat penalty against the
/// stage base time (5s for DNF/FTS/WD, 10s for a no-show).
fn run_net_score(run: &RunRecord, base_time: u16) -> u32 {
    match finish_to_ktime(run) {
        KTime::Time(t) => t.time_ds as u32 + 50 * (t.flags as u32 + t.garage as u32),
        KTime::DNF | KTime::FTS | KTime::WD => base_time as u32 + 50,
        KTime::NOSHO => base_time as u32 + 100,
    }
}

/// Per-stage base time: `min(slowest, fastest * 2)` over clean runs, the same
/// rule as before but read from the run log instead of collapsed scores.
pub fn base_times_for(event: &EventInfo, runs: &[RunRecord]) -> Vec<u16> {
    (0..event.stage_count())
        .map(|s| {
            let test = s as u8 + 1;
            let mut fastest: u32 = u16::MAX as u32;
            let mut slowest: u32 = 0;
            for run in runs
                .iter()
                .filter(|r| r.r#type == RUN_FINISH && !r.voided && r.test == test)
            {
                if let KTime::Time(t) = finish_to_ktime(run) {
                    let score = t.time_ds as u32 + 50 * (t.flags as u32 + t.garage as u32);
                    fastest = fastest.min(score);
                    slowest = slowest.max(score);
                }
            }
            if slowest == 0 {
                0
            } else {
                slowest.min(fastest * 2) as u16
            }
        })
        .collect()
}

/// Best-N-of-total aggregate for one car in one test: the sum of the car's
/// best `runs_scored` counting runs of the (up to) `runs_total` it attempted,
/// where only the first `runs_total` runs (by run order) may count — extra
/// runs beyond total are excluded no matter their time.  A car that hasn't
/// completed `runs_scored` runs yet scores nothing: the real runs are returned
/// for display but `sum` stays None.  A 0-of-0 stage (total = 0) is completed
/// by every entrant with a total time of zero — any runs recorded are
/// display-only (struck out).  DNF/FTS/WD finishes and a declared DNS (a
/// `start` marked `dns`, no finish) are completed attempts; a DNS scores the
/// no-time `base + 100`.  Returns the aggregate score together with every real
/// run (in run order, counted runs flagged).  None when the car has no attempts
/// in the test.
fn stage_result(
    stage: &Stage,
    runs: &[RunRecord],
    test: u8,
    car: &str,
    base_time: u16,
) -> Option<StageScore> {
    let relevant: Vec<&RunRecord> = runs
        .iter()
        .filter(|r| !r.voided && r.test == test && r.car == car)
        .collect();

    // A 0-of-0 stage (Y = 0): zero required runs, so every entrant has
    // completed it with a total time of zero.  Any runs recorded are
    // display-only (struck out, never counted).
    if stage.runs_total == 0 {
        let mut relevant: Vec<&RunRecord> = relevant;
        relevant.sort_by_key(|r| r.ts);
        let all: Vec<RunScore> = relevant
            .iter()
            .filter(|r| {
                r.r#type == RUN_FINISH
                    || (r.r#type == RUN_START && r.status.as_deref() == Some("dns"))
            })
            .map(|r| {
                let (time, score) = if r.r#type == RUN_FINISH {
                    (finish_to_ktime(r), run_net_score(r, base_time))
                } else {
                    (KTime::NOSHO, base_time as u32 + 100)
                };
                RunScore {
                    ts: r.ts,
                    time,
                    score,
                    counted: false,
                }
            })
            .collect();
        return Some(StageScore {
            sum: Some(0),
            runs: all,
        });
    }

    if relevant.is_empty() {
        return None; // car hasn't appeared in this test at all
    }

    let mut all: Vec<RunScore> = relevant
        .iter()
        .filter(|r| {
            r.r#type == RUN_FINISH || (r.r#type == RUN_START && r.status.as_deref() == Some("dns"))
        })
        .map(|r| {
            let (time, score) = if r.r#type == RUN_FINISH {
                (finish_to_ktime(r), run_net_score(r, base_time))
            } else {
                (KTime::NOSHO, base_time as u32 + 100)
            };
            RunScore {
                ts: r.ts,
                time,
                score,
                counted: false,
            }
        })
        .collect();
    all.sort_by_key(|r| r.ts);
    if all.is_empty() {
        return None; // only in-progress starts: nothing to show or score yet
    }

    // Completeness gate: no score until the entrant has done enough runs.
    let y = stage.runs_total as usize;
    let fill_target = if stage.runs_scored == 0 {
        y
    } else {
        (stage.runs_scored.max(1)) as usize
    };
    let done = all.len().min(y);
    if done < fill_target {
        return Some(StageScore {
            sum: None,
            runs: all,
        });
    }

    // Counting-eligible slots: the first Y runs (by timestamp order).  Beyond-Y
    // runs are excluded no matter how fast.
    let eligible: Vec<usize> = (0..all.len()).filter(|&i| i < y).collect();
    let keep = if stage.runs_scored == 0 {
        eligible.len()
    } else {
        (stage.runs_scored as usize).min(eligible.len()).max(1)
    };
    let mut best = eligible.clone();
    best.sort_by_key(|&i| (all[i].score, all[i].ts));
    for &i in best.iter().take(keep) {
        all[i].counted = true;
    }
    let total: u32 = best.iter().take(keep).map(|&i| all[i].score).sum();

    // No counting-eligible runs (cancelled stage): nothing scores, the real
    // runs are still returned for display (struck out).
    let sum = if best.is_empty() { None } else { Some(total) };

    Some(StageScore { sum, runs: all })
}

pub fn calc(rv: &mut ResultView) {
    calc_stage_positions(rv);
    calc_cumulative_times(rv);
    calc_cumulative_positions(rv);
    calc_pos_changes(rv);
}

pub fn create_result_view(event: &EventInfo, runs: &[RunRecord], class: &str) -> ResultView {
    let mut rv = ResultView::init(class, event, runs);
    calc(&mut rv);
    rv
}

/// The overall standing: every active entry, regardless of class membership.
/// Always available, so the results page can show an Outright tab even when the
/// event's classes list doesn't include one.
pub fn create_outright_view(event: &EventInfo, runs: &[RunRecord]) -> ResultView {
    let mut rv = ResultView::init("Outright", event, runs);
    if !event.classes.iter().any(|c| c == "Outright") {
        rv.rows = event
            .sorted_entries()
            .into_iter()
            .filter(|e| is_active_entry(e))
            .map(|e| {
                (
                    e.entry_no,
                    ResultRow::init(e, event, runs, &rv.base_times_ds),
                )
            })
            .collect();
    }
    calc(&mut rv);
    rv
}

/// Cumulative score per test: the running sum of completed stage scores.  An
/// entrant who hasn't completed a test (unscored, or never attempted) gets no
/// cumulative from then on — later tests still show their own Score/Pos but
/// their Cum and O/R stay blank.  A 0-of-0 stage is completed by everyone
/// (zero total), so the cumulative chain runs straight through it.
pub fn calc_cumulative_times(rv: &mut ResultView) {
    for row in rv.rows.values_mut() {
        let mut score = 0;
        let mut broken = false;
        for rs in row.columns.iter_mut() {
            if broken {
                if let Some(rs) = rs {
                    rs.cum_pos = None;
                }
                continue;
            }
            match rs {
                Some(rs) => match &rs.stage_pos {
                    Some(sp) => {
                        score += sp.score_ds;
                        rs.cum_pos = Some(Pos::init(score));
                    }
                    None => {
                        rs.cum_pos = None;
                        broken = true;
                    }
                },
                None => broken = true, // test never attempted -> no cum after this
            }
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
                if let Some(sp) = &mut rs.stage_pos {
                    positions.push(sp);
                }
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

// ---------------------------------------------------------------------------
// Event id + slug helpers.  The event id is the primary key everywhere:
// localStorage transaction log (`log:<id>` / `pending:<id>`), Matrix aliases
// and timing payloads.
// ---------------------------------------------------------------------------

/// Slugify a string for use in ids/aliases: lowercase `[a-z0-9]` joined by `-`.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // room aliases (wasm publish)
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
///
/// The event's own id is random; this slug is used for the room alias at
/// publish (wasm) and in tests.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
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
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // room-alias check (wasm publish)
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

/// A fresh, unique event id: opaque random key (`kt-<short id>`), never
/// derived from the human fields.  Name/club/year are ordinary editable fields
/// — they matter at publish, when they form the room alias.
pub fn fresh_event_id() -> String {
    format!("{ID_PREFIX}{}", crate::ids::gen_short_id())
}

/// True when `homeserver` belongs to matrix.org (matched by host, like the
/// stored-session helper in `services/matrix.rs` — pure here so it's usable
/// off-wasm too, e.g. for the Element-link default).
pub fn is_matrix_org_homeserver(homeserver: &str) -> bool {
    let host = homeserver
        .split("://")
        .nth(1)
        .unwrap_or(homeserver)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(homeserver);
    let host = host.split(':').next().unwrap_or(host).to_lowercase();
    host == "matrix.org" || host.ends_with(".matrix.org")
}

/// Default Element Web origin for `homeserver` (used when the event has no
/// explicit `element_link`): app.element.io for Matrix, else the local Element
/// dev instance.  Empty for an unknown/blank homeserver.
pub fn element_link_default(homeserver: &str) -> String {
    let hs = homeserver.trim();
    if hs.is_empty() {
        return String::new();
    }
    if is_matrix_org_homeserver(hs) {
        "https://app.element.io".to_string()
    } else {
        "http://localhost:8085".to_string()
    }
}

/// Extract the Matrix server name (host, optionally with port) from a
/// homeserver URL, suitable for building a room alias:
/// `http://localhost:8008` → `"localhost:8008"`, `https://matrix.org` →
/// `"matrix.org"`.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // wasm invite_view_data
pub fn server_name_from_homeserver(homeserver: &str) -> String {
    let hs = homeserver.trim();
    if hs.is_empty() {
        return String::new();
    }
    let hostport = hs
        .split("://")
        .nth(1)
        .unwrap_or(hs)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(hs);
    hostport.to_string()
}

// ---------------------------------------------------------------------------
// Invite URL.  `?homeserver=..&event=<id>&sid=<space id>&tid=<timing id>&reg=..`
// Room ids only — no aliases, no fallbacks.  Event details come from the room.
// ---------------------------------------------------------------------------

/// QR / URL invite for joining a published event.  Minimal and self-contained:
/// enough to connect, adopt by room id and sign in per `reg`; the event's full
/// setup arrives from the room afterwards.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Invite {
    pub homeserver: String,
    pub event: String, // event id
    pub sid: String,   // space room id
    pub tid: String,   // timing room id
    pub reg: RegistrationMode,
}

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

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

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

impl Invite {
    pub fn to_query(&self) -> String {
        format!(
            "homeserver={}&event={}&sid={}&tid={}&reg={}",
            encode_query(&self.homeserver),
            encode_query(&self.event),
            encode_query(&self.sid),
            encode_query(&self.tid),
            encode_query(reg_str(self.reg)),
        )
    }

    pub fn from_query(q: &str) -> Option<Invite> {
        let mut homeserver = None;
        let mut event = None;
        let mut sid = None;
        let mut tid = None;
        let mut reg = None;
        for pair in q.split('&') {
            let mut it = pair.splitn(2, '=');
            let (k, v) = (it.next()?, it.next()?);
            match k {
                "homeserver" => homeserver = Some(decode_query(v)),
                "event" => event = Some(decode_query(v)),
                "sid" => sid = Some(decode_query(v)),
                "tid" => tid = Some(decode_query(v)),
                "reg" => reg = Some(decode_query(v)),
                _ => {}
            }
        }
        Some(Invite {
            homeserver: homeserver.unwrap_or_default(),
            event: event?,
            sid: sid.unwrap_or_default(),
            tid: tid.unwrap_or_default(),
            // Default to SSO (conservative) on an unknown/absent reg.
            reg: reg.and_then(|r| reg_from_str(&r)).unwrap_or_default(),
        })
    }

    /// Absolute invite URL for `app_base` (origin + path, e.g.
    /// `https://host/khanatime/`).
    pub fn url(&self, app_base: &str) -> String {
        format!("{app_base}?{}", self.to_query())
    }

    /// Parse a pasted join link — an absolute URL (`https://host/app?…`) or a
    /// bare query string (`homeserver=…&event=…`) — into an [Invite].  Fields
    /// may be empty; the caller validates completeness.
    pub fn from_url(s: &str) -> Option<Invite> {
        let query = match s.rfind('?') {
            Some(i) => &s[i + 1..],
            None => s,
        };
        Invite::from_query(query)
    }
}

fn reg_str(mode: RegistrationMode) -> &'static str {
    match mode {
        RegistrationMode::Open => "open",
        RegistrationMode::Sso => "sso",
    }
}

fn reg_from_str(s: &str) -> Option<RegistrationMode> {
    match s {
        "open" => Some(RegistrationMode::Open),
        "sso" => Some(RegistrationMode::Sso),
        _ => None,
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
const EVENT_RECENT: &str = "event_recent";

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
    let (ev, _, _) =
        crate::replay::replay(&crate::log::load_log(key), &crate::log::load_pending(key));
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
        stages: (1..=4).map(Stage::for_test).collect(),
        classes: ["Outright", "Female", "Junior"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        ..Default::default()
    };
    // Stage 2 is the multi-run test: best 2 of 3 (the others are single runs).
    ev.stages[1].runs_total = 3;
    ev.stages[1].runs_scored = 2;
    // Stage 3 is 0 of 0: everyone completes it with a total time of zero, so
    // positions tie and the cumulative chain continues on to stage 4.
    ev.stages[2].runs_total = 0;
    ev.stages[2].runs_scored = 0;
    // Stage 4 is a normal single run after the zero stage.
    ev.stages[3].runs_total = 1;
    ev.stages[3].runs_scored = 1;
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
    ev.ensure_uid();
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

/// The `khanatime_setup:` manifest body for an event.
pub fn setup_body(ev: &EventInfo) -> String {
    format!(
        "{}{}",
        crate::timing_event::TimingEvent::SETUP_PREFIX,
        serde_json::to_string(ev).unwrap()
    )
}

/// Enqueue an event-setup manifest into the event's outbox (publisher side).
pub fn enqueue_event_setup(ev: &EventInfo) {
    if ev.id.is_empty() {
        return;
    }
    crate::log::enqueue_pending(
        &ev.id,
        crate::log::LogMsg::new_pending(setup_body(ev), String::new()),
    );
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
    if event.homeserver.trim().is_empty() {
        errs.push("Pick a homeserver to publish to.".to_string());
    }
    // The human fields form the room alias at publish, so they must be usable
    // even though the event id itself is random.
    if event.name.trim().is_empty() || year_token(&event.year).is_empty() {
        errs.push(
            "Add the event name and a 4-digit year before publishing (they form the room alias)."
                .to_string(),
        );
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

/// Append a run, skipping one whose observation `uid` is already present
/// (the same observation mirrored by room, relay or QR collapses to one).
/// Returns true when added.
pub fn add_run(runs: &mut Vec<RunRecord>, run: RunRecord) -> bool {
    if runs.iter().any(|r| r.uid == run.uid) {
        return false;
    }
    runs.push(run);
    true
}

/// The run record with observation id `uid`, if present.
pub fn find_run<'a>(runs: &'a [RunRecord], uid: &str) -> Option<&'a RunRecord> {
    runs.iter().find(|r| r.uid == uid)
}

/// Cars that started `test` but have no finish yet, oldest first.
/// A start is "matched" when a finish record references its uid in `refs`.
pub fn pending_starts(runs: &[RunRecord], test: u8) -> Vec<&RunRecord> {
    let mut out: Vec<&RunRecord> = runs
        .iter()
        .filter(|r| r.r#type == RUN_START && r.test == test)
        .filter(|r| r.status.as_deref() != Some("dns"))
        .filter(|r| !r.voided)
        .filter(|r| {
            !runs
                .iter()
                .any(|f| f.r#type == RUN_FINISH && !f.voided && f.refs.contains(&r.uid))
        })
        .collect();
    out.sort_by_key(|r| r.ts);
    out
}

/// Whether `car` has an unfinished (pending) start for `test`.
pub fn pending_for_car(runs: &[RunRecord], test: u8, car: &str) -> bool {
    pending_starts(runs, test).iter().any(|r| r.car == car)
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
        uid: te.uid.clone(),
        r#type: te.r#type.clone(),
        test: te.test,
        car: te.car.clone(),
        ts: te.ts,
        time_ds: te.time_ds,
        status: te.status.clone(),
        flags: te.flags,
        official_id: te.official_id.clone(),
        comment: te.comment.clone(),
        refs: te.refs.clone(),
        voided: false,
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

/// Clear the current-event session pointer (used when the open event is
/// deleted), leaving the app back in the "no event" mode.
pub fn session_clear_event() {
    if let Some(st) = session_storage() {
        let _ = st.remove_item(EVENT_SESSION);
    }
}

/// The id of the most recently opened event (for the picker's "Recent" tag).
pub fn session_recent_event() -> String {
    session_storage()
        .and_then(|st| st.get_item(EVENT_RECENT).ok().flatten())
        .unwrap_or_default()
}

/// Record `key` as the most recently opened event.
pub fn session_set_recent(key: &str) {
    if !key.is_empty() {
        if let Some(st) = session_storage() {
            let _ = st.set_item(EVENT_RECENT, key);
        }
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
            homeserver: "https://matrix-client.matrix.org".to_string(),
            event: "kt-2026-ndc-kcr1".to_string(),
            sid: "!abc:matrix.org".to_string(),
            tid: "!xyz:matrix.org".to_string(),
            reg: RegistrationMode::Sso,
        };
        let q = inv.to_query();
        assert!(q.contains("sid=%21abc"));
        assert!(q.contains("reg=sso"));
        let back = Invite::from_query(&q).expect("parses");
        assert_eq!(back, inv);
    }

    #[test]
    fn invite_open_reg_round_trip() {
        let inv = Invite {
            homeserver: "http://192.168.1.10:8008".to_string(),
            event: "kt-2026-x".to_string(),
            sid: "!a:b".to_string(),
            tid: "!b:b".to_string(),
            reg: RegistrationMode::Open,
        };
        let back = Invite::from_query(&inv.to_query()).unwrap();
        assert_eq!(back.reg, RegistrationMode::Open);
    }

    #[test]
    fn invite_defaults_to_sso_and_missing_event_none() {
        // Absent reg -> conservative sso.
        let q = "homeserver=http://h&event=kt-2026&sid=!a&tid=!b";
        assert_eq!(Invite::from_query(q).unwrap().reg, RegistrationMode::Sso);
        // Missing event -> None.
        assert!(Invite::from_query("homeserver=http://h&sid=!a").is_none());
        // Unknown reg -> sso.
        let q = "homeserver=http://h&event=e&sid=!a&tid=!b&reg=warp";
        assert_eq!(Invite::from_query(q).unwrap().reg, RegistrationMode::Sso);
    }

    #[test]
    fn invite_from_url_accepts_absolute_and_bare_query() {
        let inv = Invite {
            homeserver: "https://matrix.org".into(),
            event: "ev1".into(),
            sid: "!space:matrix.org".into(),
            tid: "!timing:matrix.org".into(),
            reg: RegistrationMode::Sso,
        };
        let absolute = format!("https://host/app?{}", inv.to_query());
        assert_eq!(Invite::from_url(&absolute).unwrap(), inv);
        assert_eq!(Invite::from_url(&inv.to_query()).unwrap(), inv);
        // Missing event id fails to parse.
        assert!(Invite::from_url("homeserver=https://matrix.org&sid=!a&tid=!b").is_none());
    }

    #[test]
    fn invite_url_builds_absolute() {
        let inv = Invite {
            homeserver: "http://localhost:8008".to_string(),
            event: "kt-2026".to_string(),
            sid: "!s:localhost".to_string(),
            tid: "!t:localhost".to_string(),
            reg: RegistrationMode::Open,
        };
        assert!(inv
            .url("http://localhost:8080/khanatime/")
            .starts_with("http://localhost:8080/khanatime/?"));
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

    fn run(r#type: &str, test: u8, car: &str, ts: i64) -> RunRecord {
        let t = r#type;
        RunRecord {
            uid: format!("uid-{t}-{ts}"),
            r#type: t.into(),
            test,
            car: car.into(),
            ts,
            ..Default::default()
        }
    }

    #[test]
    fn add_run_dedupes() {
        let mut runs = vec![run("start", 1, "7", 100)];
        assert!(!add_run(&mut runs, run("start", 1, "7", 100)));
        assert!(add_run(&mut runs, run("start", 1, "7", 200)));
        assert_eq!(runs.len(), 2);
    }

    #[test]
    fn pending_starts_hides_finished_and_dns() {
        let mut runs = vec![
            run("start", 1, "7", 100),
            run("start", 1, "8", 200),
            run("start", 1, "9", 300),
            run("finish", 1, "8", 400),
        ];
        // Finish for car 8 references start uid "uid-start-200"
        runs[3].refs = vec!["uid-start-200".into()];
        let mut dns = run("start", 1, "9", 300);
        dns.status = Some("dns".into());
        runs[2] = dns;
        let pending = pending_starts(&runs, 1);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].car, "7");
        assert!(pending_starts(&runs, 2).is_empty());
    }

    #[test]
    fn pending_for_car_true_when_unfinished() {
        let mut runs = vec![
            run("start", 1, "7", 100),
            run("start", 1, "8", 200),
            run("finish", 1, "8", 300),
        ];
        // Finish for car 8 references start uid "uid-start-200"
        runs[2].refs = vec!["uid-start-200".into()];
        assert!(pending_for_car(&runs, 1, "7"));
        assert!(!pending_for_car(&runs, 1, "8"));
        assert!(!pending_for_car(&runs, 2, "7"));
    }

    #[test]
    fn elapsed_ds_rounds() {
        assert_eq!(elapsed_ds(0, 0), 0);
        assert_eq!(elapsed_ds(1000, 1000 + 12350), 124); // 12.35s -> 124ds
        assert_eq!(elapsed_ds(1000, 500), 0);
    }

    #[test]
    fn finish_to_ktime_maps_status() {
        let mut c = run("finish", 1, "7", 0);
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
        let mut dnf = run("finish", 1, "7", 0);
        dnf.status = Some("dnf".into());
        assert_eq!(finish_to_ktime(&dnf), KTime::DNF);
        let mut g = run("finish", 1, "7", 0);
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
            uid: "ABCDEFGHJK".into(),
            target: None,
            test: 2,
            car: "17".into(),
            ts: 42,
            time_ds: Some(999),
            status: Some("clean".into()),
            flags: Some(1),
            official_id: Some("u".into()),
            comment: None,
            refs: vec![],
        };
        let r = record_from_timing(&te);
        assert_eq!(r.uid, "ABCDEFGHJK");
        assert!(!r.voided);
        assert_eq!(r.r#type, "finish");
        assert_eq!(r.test, 2);
        assert_eq!(r.car, "17");
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
        assert_eq!(demo.stages.len(), 4);
        assert!(!demo.entries.is_empty());
        assert!(!demo.classes.is_empty());
        // A demo event carries no timing data.
        assert!(!has_timing_data(&[], &[]));
    }

    #[test]
    fn publish_errors_catch_bad_state() {
        let mut ev = EventInfo {
            id: crate::ids::gen_short_id(),
            name: "X".into(),
            year: "2026".into(),
            stages: vec![],
            homeserver: "http://localhost:8008".into(),
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
    fn stage_result_runs_scored_of_total() {
        let ev = EventInfo::default();
        let stage = Stage {
            num: 1,
            runs_total: 3,
            runs_scored: 2,
            ..ev.stage(0).clone()
        };
        let finish = |ith: u8, ds: u16| RunRecord {
            r#type: "finish".into(),
            test: 1,
            car: "7".into(),
            ts: ith as i64,
            time_ds: Some(ds),
            ..Default::default()
        };
        let runs = vec![finish(1, 200), finish(2, 100), finish(3, 300)];
        // Best 2 of 3 count: 100 + 200 = 300.
        assert_eq!(
            stage_result(&stage, &runs, 1, "7", 0).and_then(|ss| ss.sum),
            Some(300)
        );
        let mut best1 = stage.clone();
        best1.runs_scored = 1;
        assert_eq!(
            stage_result(&best1, &runs, 1, "7", 0).and_then(|ss| ss.sum),
            Some(100)
        );
        let mut best0 = stage.clone();
        best0.runs_scored = 0; // count all runs up to repeats
        assert_eq!(
            stage_result(&best0, &runs, 1, "7", 0).and_then(|ss| ss.sum),
            Some(600)
        );
    }

    #[test]
    fn stage_result_beyond_y_runs_are_excluded() {
        let ev = EventInfo::default();
        let stage = Stage {
            num: 1,
            runs_total: 3, // Y = 3: only the first three runs may count
            runs_scored: 2,
            ..ev.stage(0).clone()
        };
        let finish = |ith: u8, ds: u16| RunRecord {
            r#type: "finish".into(),
            test: 1,
            car: "7".into(),
            ts: ith as i64,
            time_ds: Some(ds),
            ..Default::default()
        };
        // A very fast 4th run must NOT steal a counting slot from the first Y.
        let runs = vec![
            finish(1, 400),
            finish(2, 500),
            finish(3, 600),
            finish(4, 50),
        ];
        let ss = stage_result(&stage, &runs, 1, "7", 0).unwrap();
        assert_eq!(ss.sum, Some(900)); // best 2 of the first 3, run 4 ignored
        let shown: Vec<(i64, bool)> = ss.runs.iter().map(|r| (r.ts, r.counted)).collect();
        assert_eq!(shown, vec![(1, true), (2, true), (3, false), (4, false)]);
    }

    #[test]
    fn stage_result_no_score_until_enough_attempts() {
        let ev = EventInfo::default();
        let stage = Stage {
            num: 1,
            runs_total: 3, // Y = 3
            runs_scored: 2,
            ..ev.stage(0).clone()
        };
        let finish = |ith: u8, ds: u16| RunRecord {
            r#type: "finish".into(),
            test: 1,
            car: "7".into(),
            ts: ith as i64,
            time_ds: Some(ds),
            ..Default::default()
        };

        // One run of best-2-of-3: the real run is shown but scores nothing.
        let ss = stage_result(&stage, &[finish(1, 450)], 1, "7", 0).unwrap();
        assert_eq!(ss.sum, None);
        let shown: Vec<(i64, u32, bool)> =
            ss.runs.iter().map(|r| (r.ts, r.score, r.counted)).collect();
        assert_eq!(shown, vec![(1, 450, false)]);

        // Two runs of best-2-of-3: enough attempts, both count.
        let ss = stage_result(&stage, &[finish(1, 450), finish(2, 470)], 1, "7", 0).unwrap();
        assert_eq!(ss.sum, Some(920));
        assert!(ss.runs.iter().all(|r| r.counted));

        // A DNF is a completed attempt: clean + DNF scores both.
        let mut dnf = finish(2, 0);
        dnf.status = Some("dnf".into());
        let ss = stage_result(&stage, &[finish(1, 450), dnf], 1, "7", 0).unwrap();
        assert_eq!(ss.sum, Some(500)); // 450 + DNF(50)

        // A declared DNS (start marked dns, no finish) is a completed attempt
        // scoring the no-time: clean + DNS is enough for best-2-of-3.
        let dns_start = RunRecord {
            r#type: "start".into(),
            test: 1,
            car: "7".into(),
            ts: 2,
            status: Some("dns".into()),
            ..Default::default()
        };
        let ss = stage_result(&stage, &[finish(1, 450), dns_start], 1, "7", 0).unwrap();
        assert_eq!(ss.sum, Some(550)); // 450 + DNS(100)

        // No runs at all stays blank (None).
        assert!(stage_result(&stage, &[], 1, "7", 0).is_none());

        // Only an in-progress start: nothing to show or score yet.
        let pending = RunRecord {
            r#type: "start".into(),
            test: 1,
            car: "7".into(),
            ts: 1,
            ..Default::default()
        };
        assert!(stage_result(&stage, &[pending], 1, "7", 0).is_none());
    }

    #[test]
    fn stage_result_status_and_dns() {
        let ev = EventInfo::default();
        let stage = ev.stage(0); // runs_total=1, runs_scored=1
        let run = |ith: u8, time_ds, flags| RunRecord {
            r#type: "finish".into(),
            test: 1,
            car: "7".into(),
            ts: ith as i64,
            time_ds: Some(time_ds),
            flags: Some(flags),
            ..Default::default()
        };
        // A clean finish wins even when a slower one has lawn-dart flag penalties.
        assert_eq!(
            stage_result(&stage, &[run(1, 450, 0)], 1, "7", 0).and_then(|ss| ss.sum),
            Some(450)
        );
        // Declared DNS (start marked dns, no finish) on a 1:1 stage: a real
        // recorded attempt scoring the no-time base + 100.
        let dnss = vec![RunRecord {
            r#type: "start".into(),
            test: 1,
            car: "9".into(),
            ts: 1,
            status: Some("dns".into()),
            ..Default::default()
        }];
        assert_eq!(
            stage_result(&stage, &dnss, 1, "9", 1000).and_then(|ss| ss.sum),
            Some(1100)
        );
    }

    #[test]
    fn default_event_has_stages() {
        let ev = EventInfo::default();
        // A fresh event starts with a single test; the organiser adds more.
        assert_eq!(ev.stage_count(), 1);
        let first = ev.stage(0);
        assert_eq!(first.num, 1);
        assert_eq!(first.name, "Test 1");
        assert_eq!(first.runs_total, 1);
        assert_eq!(first.runs_scored, 1);
        assert_eq!(first.timing, TimingStyle::Stopwatch);
    }

    #[test]
    fn stages_roundtrip_via_json() {
        let ev = EventInfo {
            stages: vec![
                Stage::for_test(1),
                Stage {
                    num: 2,
                    name: "Creek".into(),
                    runs_total: 3,
                    runs_scored: 2,
                    timing: TimingStyle::Rally,
                },
            ],
            ..Default::default()
        };
        let json = serde_json::to_string(&ev).unwrap();
        let back: EventInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.stage_count(), 2);
        assert_eq!(back.stage(1).name, "Creek");
        assert_eq!(back.stage(1).runs_total, 3);
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

        // Legacy entries (no status field) read as the default (submitted).
        let legacy = r#"{"car":"8","name":"Bob","classes":["Outright"]}"#;
        let back: Entry = serde_json::from_str(legacy).unwrap();
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
            stages: vec![Stage::for_test(1), Stage::for_test(2)],
            ..Default::default()
        };
        let finish = |test: u8, car: &str, ith: u8, ds: u16| RunRecord {
            r#type: "finish".into(),
            test,
            car: car.into(),
            ts: ith as i64,
            time_ds: Some(ds),
            ..Default::default()
        };
        let runs = vec![
            finish(1, "1", 1, 450),
            finish(2, "1", 1, 470),
            finish(1, "2", 1, 500),
            finish(2, "2", 1, 520),
        ];
        let rv = create_result_view(&ev, &runs, "Outright");
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
        assert_eq!(alice.len(), 2);
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
    fn demo_runs_scored_of_total_sums_runs_on_stage() {
        let ev = demo_event();
        // Stage 2 ships as best-2-of-3; exercise that configuration.
        assert_eq!(ev.stages[1].runs_total, 3);
        assert_eq!(ev.stages[1].runs_scored, 2);
        let finish = |ith: u8, ds: u16| RunRecord {
            r#type: "finish".into(),
            test: 2,
            car: "1".into(),
            ts: ith as i64,
            time_ds: Some(ds),
            ..Default::default()
        };
        let runs = vec![finish(1, 450), finish(2, 470), finish(3, 100)];
        let rv = create_result_view(&ev, &runs, "Outright");
        let alice_entry_no = ev.entries[0].entry_no;
        let stage2 = &rv.rows[&alice_entry_no].columns[1].as_ref().unwrap();
        // Best 2 of 3 = 450 + 100 = 550.
        assert_eq!(stage2.stage_pos.as_ref().unwrap().score_ds, 550);
        // Display order is run order, with the non-counting run struck out.
        let shown: Vec<(i64, u32, bool)> = stage2
            .runs
            .iter()
            .map(|r| (r.ts, r.score, r.counted))
            .collect();
        assert_eq!(shown, vec![(1, 450, true), (2, 470, false), (3, 100, true)]);
    }

    #[test]
    fn demo_event_has_zero_run_stage() {
        let ev = demo_event();
        assert_eq!(ev.stage_count(), 4);
        // Stage 2 stays the multi-run test.
        assert_eq!(ev.stages[1].runs_total, 3);
        assert_eq!(ev.stages[1].runs_scored, 2);
        // Stage 3 is 0 of 0: everyone completes it with a zero total.
        assert_eq!(ev.stages[2].runs_total, 0);
        assert_eq!(ev.stages[2].runs_scored, 0);
        // Stage 4 is a normal single run after the zero stage.
        assert_eq!(ev.stages[3].runs_total, 1);
        assert_eq!(ev.stages[3].runs_scored, 1);
    }

    #[test]
    fn zero_run_stage_scores_zero_and_shows_runs() {
        let ev = EventInfo::default();
        let stage = Stage {
            num: 3,
            runs_total: 0, // 0 of 0: everyone completes with a zero total
            runs_scored: 0,
            ..ev.stage(0).clone()
        };
        let finish = RunRecord {
            r#type: "finish".into(),
            test: 3,
            car: "7".into(),
            ts: 1,
            time_ds: Some(450),
            ..Default::default()
        };
        // A recorded run is shown for display (struck out) but the stage score
        // is zero regardless.
        let ss = stage_result(&stage, &[finish], 3, "7", 0).unwrap();
        assert_eq!(ss.sum, Some(0));
        assert_eq!(ss.runs.len(), 1);
        assert!(!ss.runs[0].counted);
        // An entrant who never appeared also completed it with a zero total.
        let none = stage_result(&stage, &[], 3, "9", 0).unwrap();
        assert_eq!(none.sum, Some(0));
        assert!(none.runs.is_empty());
    }

    #[test]
    fn zero_stage_propagates_positions_and_cumulative() {
        let mut a = Entry::new("1", "Alice");
        a.entry_no = 1;
        let mut b = Entry::new("2", "Bob");
        b.entry_no = 2;
        // Stage 2 becomes 0 of 0; stages 1 and 3 stay normal single runs.
        let mut stages: Vec<Stage> = (1..=3).map(Stage::for_test).collect();
        stages[1].runs_total = 0;
        stages[1].runs_scored = 0;
        let ev = EventInfo {
            entries: vec![a, b],
            stages,
            ..Default::default()
        };
        let finish = |test: u8, car: &str, ds: u16| RunRecord {
            r#type: "finish".into(),
            test,
            car: car.into(),
            ts: test as i64,
            time_ds: Some(ds),
            ..Default::default()
        };
        let runs = vec![
            finish(1, "1", 450),
            finish(3, "1", 600),
            finish(1, "2", 500),
            finish(3, "2", 300),
        ];
        let rv = create_result_view(&ev, &runs, "Outright");
        let alice = &rv.rows[&1u32].columns;
        let bob = &rv.rows[&2u32].columns;
        // Stage 2: everyone ties on a zero total.
        for row in [alice, bob] {
            let s2 = row[1].as_ref().unwrap();
            let sp = s2.stage_pos.as_ref().unwrap();
            assert_eq!(sp.score_ds, 0);
            assert_eq!(sp.pos, 1);
        }
        // Cumulative runs through the zero stage: 450 + 0 + 600 = 1050, and
        // 500 + 0 + 300 = 800.
        assert_eq!(
            alice[2]
                .as_ref()
                .unwrap()
                .cum_pos
                .as_ref()
                .unwrap()
                .score_ds,
            1050
        );
        assert_eq!(
            bob[2].as_ref().unwrap().cum_pos.as_ref().unwrap().score_ds,
            800
        );
    }

    #[test]
    fn cumulative_chain_breaks_on_missing_test() {
        let mut a = Entry::new("1", "Alice");
        a.entry_no = 1;
        let mut b = Entry::new("2", "Bob");
        b.entry_no = 2;
        let ev = EventInfo {
            entries: vec![a, b],
            stages: (1..=3).map(Stage::for_test).collect(),
            ..Default::default()
        };
        let finish = |test: u8, car: &str, ith: u8, ds: u16| RunRecord {
            r#type: "finish".into(),
            test,
            car: car.into(),
            ts: ith as i64,
            time_ds: Some(ds),
            ..Default::default()
        };
        // Alice misses test 2 entirely; Bob completes all three tests.
        let runs = vec![
            finish(1, "1", 1, 450),
            finish(3, "1", 1, 600),
            finish(1, "2", 1, 500),
            finish(2, "2", 1, 520),
            finish(3, "2", 1, 300),
        ];
        let rv = create_result_view(&ev, &runs, "Outright");
        let alice = &rv.rows[&1u32].columns;
        // Test 3 is scored (she ran it), but its Cum/O-R is blank because
        // test 2 was never completed.
        let t3 = alice[2].as_ref().unwrap();
        assert_eq!(t3.stage_pos.as_ref().unwrap().score_ds, 600);
        assert!(t3.cum_pos.is_none());
        // Bob's chain runs through all three tests.
        let bob = &rv.rows[&2u32].columns;
        assert_eq!(
            bob[2].as_ref().unwrap().cum_pos.as_ref().unwrap().score_ds,
            1320
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

    #[test]
    fn server_name_from_homeserver_extracts_host_port() {
        assert_eq!(
            server_name_from_homeserver("http://localhost:8008"),
            "localhost:8008"
        );
        assert_eq!(
            server_name_from_homeserver("https://matrix.example.com"),
            "matrix.example.com"
        );
        assert_eq!(
            server_name_from_homeserver("https://matrix.org"),
            "matrix.org"
        );
        assert_eq!(server_name_from_homeserver(""), "");
    }

    #[test]
    fn element_link_default_per_homeserver() {
        assert_eq!(
            element_link_default("https://matrix.org"),
            "https://app.element.io"
        );
        assert_eq!(
            element_link_default("http://localhost:8008"),
            "http://localhost:8085"
        );
        assert_eq!(element_link_default(""), "");
    }
}
