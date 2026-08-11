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
#[derive(Debug, Serialize, Deserialize, Clone)]
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
    pub car: String,  // entry/car number
    pub name: String, // name
    #[serde(default)]
    pub vehicle: String, // description
    #[serde(default)]
    pub classes: Vec<String>, // Classes. Count be an ID. meh
    #[serde(default)]
    pub licence: Option<String>,
    #[serde(default)]
    pub passenger: Option<String>,
    #[serde(default)]
    pub status: EntryStatus,
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

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
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
    pub rows: IndexMap<String, ResultRow>, // list of know entrants/drivers. Ordered by car number
    pub base_times_ds: Vec<u16>,           // base times

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

        let entry = Entry::new(car, name);
        self.entries.push(entry);
        true
    }

    /// Set the lifecycle status of an entry by car number.
    pub fn set_entry_status(&mut self, car: &str, status: EntryStatus) -> bool {
        if let Some(e) = self.entries.iter_mut().find(|e| e.car == *car) {
            e.status = status;
            true
        } else {
            false
        }
    }

    // delete an entry by car number
    pub fn remove_entry(&mut self, car: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.car != *car);
        before != self.entries.len()
    }
}

impl Entry {
    pub fn new(car: &str, name: &str) -> Self {
        let vehicle = Default::default();
        let classes = ["Outright"];
        let classes = classes.map(String::from).into();
        let car = car.to_string();
        let name = name.to_string();
        Self {
            vehicle,
            classes,
            car,
            name,
            licence: None,
            passenger: None,
            status: EntryStatus::Submitted,
        }
    }
}

impl<'a> ResultView {
    pub fn init(class: &str, event: &'a EventInfo, scores: &[ScoreData]) -> Self {
        let entries = find_entries_in_class(&event.entries, class);

        let rows: IndexMap<String, ResultRow> = entries
            .iter()
            .map(|e| (e.car.clone(), ResultRow::init(e, event, scores)))
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
        // collect pairs, rowkey (car) vs time
        // could collect a mut pos & too?
        let mut car_scores: Vec<(&str, &mut Pos)> = vec![];
        for (rowkey, rr) in rv.rows.iter_mut() {
            if let Some(rs) = &mut rr.columns[stage] {
                // if let Some(cum_pos) = &mut rs.cum_pos {
                car_scores.push((rowkey.as_str(), &mut rs.stage_pos));
            }
        }

        calc_rank(&mut car_scores);
    }
}

fn calc_rank(car_scores: &mut Vec<(&str, &mut Pos)>) {
    // sort by score
    car_scores.sort_unstable_by_key(|a| a.1.score_ds);

    // calc the ranks and eq and poke into the cum_pos Pos
    let mut last_time = 0u16;
    let mut rank = 1u8;
    for (idx, (_, pos)) in car_scores.iter_mut().enumerate() {
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
        // collect pairs, rowkey (car) vs time
        // could collect a mut pos & too?
        let mut car_scores: Vec<(&str, &mut Pos)> = vec![];
        for (rowkey, rr) in rv.rows.iter_mut() {
            if let Some(rs) = &mut rr.columns[stage] {
                if let Some(cum_pos) = &mut rs.cum_pos {
                    car_scores.push((rowkey.as_str(), cum_pos));
                }
            }
        }
        calc_rank(&mut car_scores);
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
    let mut car_totals: Vec<(&str, u32, &mut ResultRow)> = vec![];
    for (key, row) in rv.rows.iter_mut() {
        if row.total_ds == 0 {
            continue; // no completed runs yet
        }
        car_totals.push((key.as_str(), row.total_ds, row));
    }
    car_totals.sort_by_key(|a| a.1);
    let mut last = u32::MAX;
    let mut rank = 1u8;
    for (idx, (_, score, row)) in car_totals.iter_mut().enumerate() {
        let eq = *score == last;
        last = *score;
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
            .entries
            .iter()
            .filter(|e| is_active_entry(e))
            .map(|e| (e.car.clone(), ResultRow::init(e, event, scores)))
            .collect();
    }
    calc(&mut rv);
    rv
}

// get entries  in class
pub fn find_entries_in_class<'a>(entries: &'a [Entry], class: &str) -> Vec<&'a Entry> {
    entries
        .iter()
        .filter(|e| e.classes.iter().any(|c| c == class) && is_active_entry(e))
        .collect()
}

/// Entries that count in the results: withdrawn / draft / reserve are out.
fn is_active_entry(e: &Entry) -> bool {
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

const EVENT_PREFIX: &str = "event:";
const TIMES_PREFIX: &str = "times:";
const EVENT_SESSION: &str = "event";

fn event_key(key: &str) -> String {
    format!("{}{}", EVENT_PREFIX, key)
}

fn times_key(key: &str) -> String {
    format!("{}{}", TIMES_PREFIX, key)
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn session_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.session_storage().ok().flatten()
}

fn get_json<T: serde::de::DeserializeOwned>(key: &str) -> Option<T> {
    storage()?
        .get_item(key)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn set_json<T: Serialize>(key: &str, value: &T) {
    if let Some(st) = storage() {
        let _ = st.set_item(key, &serde_json::to_string(value).unwrap());
    }
}

/// Load an event by id.  Returns the null event (empty id) when the key is
/// empty or nothing is stored under it.
pub fn load_event(key: &str) -> EventInfo {
    if key.is_empty() {
        return EventInfo {
            name: key.to_string(),
            ..Default::default()
        };
    }
    get_json(&event_key(key)).unwrap_or_default()
}

/// Key that [load_event]/[save_event]/[load_times] should use for `event`.
pub fn storage_key(event: &EventInfo) -> String {
    event.id.clone()
}

pub fn save_event(event: &EventInfo) {
    // A null event (nothing selected) must never be persisted.
    if event.id.is_empty() {
        return;
    }
    set_json(&event_key(&storage_key(event)), event);
}

/// Delete an event and all of its per-event data (times, run records).
#[allow(dead_code)] // unused until the event-deletion UI lands
pub fn remove_event(id: &str) {
    if id.is_empty() {
        return;
    }
    let Some(st) = storage() else {
        return;
    };
    let _ = st.remove_item(&event_key(id));
    let _ = st.remove_item(&times_key(id));
    let _ = st.remove_item(&runs_key(id));
    if session_event_name() == id {
        session_set_event("");
    }
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
    ] {
        ev.add_entry(car, name);
        if let Some(entry) = ev.entries.iter_mut().find(|e| e.car == car) {
            entry.classes = classes.iter().map(|s| s.to_string()).collect();
        }
    }
    ev
}

/// Restore the demo event to its pristine template, wiping all training state
/// (entries, stages, times, runs) added while practising.
pub fn reset_demo() {
    remove_event(DEMO_EVENT_ID);
    save_event(&demo_event());
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

/// List of known events in storage (ids).  If it fails .. empty is fine.
pub fn list_events() -> HashSet<String> {
    let mut out: HashSet<String> = Default::default();
    if let Some(st) = storage() {
        if let Ok(len) = st.length() {
            (0..len).for_each(|i| {
                if let Ok(Some(name)) = st.key(i) {
                    if let Some(key) = name.strip_prefix(EVENT_PREFIX) {
                        if let Some(e) = get_json::<EventInfo>(&name) {
                            out.insert(if e.id.is_empty() {
                                key.to_string()
                            } else {
                                e.id
                            });
                        }
                    }
                }
            });
        }
    }
    out
}

pub fn load_times(key: &str) -> Vec<ScoreData> {
    if key.is_empty() {
        return vec![];
    }
    get_json(&times_key(key)).unwrap_or_default()
}

pub fn save_times(key: &str, scores: &Vec<ScoreData>) {
    if !key.is_empty() {
        set_json(&times_key(key), scores);
    }
}

/// Migrate per-event data (times, run records) stored under a legacy name key
/// to the current id key.  No-op when the keys match or either is empty.
pub fn migrate_times_if_needed(old_key: &str, new_key: &str) {
    if old_key.is_empty() || new_key.is_empty() || old_key == new_key {
        return;
    }
    if get_json::<Vec<ScoreData>>(&times_key(new_key)).is_none() {
        if let Some(times) = get_json::<Vec<ScoreData>>(&times_key(old_key)) {
            set_json(&times_key(new_key), &times);
            if let Some(st) = storage() {
                let _ = st.remove_item(&times_key(old_key));
            }
        }
    }
    if get_json::<Vec<RunRecord>>(&runs_key(new_key)).is_none() {
        if let Some(runs) = get_json::<Vec<RunRecord>>(&runs_key(old_key)) {
            set_json(&runs_key(new_key), &runs);
            if let Some(st) = storage() {
                let _ = st.remove_item(&runs_key(old_key));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Run records (start/finish pairing, run numbering, pending starts).
// ---------------------------------------------------------------------------

const RUNS_PREFIX: &str = "runs:";

fn runs_key(key: &str) -> String {
    format!("{}{}", RUNS_PREFIX, key)
}

pub fn load_runs(key: &str) -> Vec<RunRecord> {
    if key.is_empty() {
        return vec![];
    }
    get_json(&runs_key(key)).unwrap_or_default()
}

pub fn save_runs(key: &str, runs: &Vec<RunRecord>) {
    if !key.is_empty() {
        set_json(&runs_key(key), runs);
    }
}

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
        a.classes = vec!["Female".into()];
        let mut b = Entry::new("2", "Bob");
        b.classes = vec!["Junior".into()];
        let mut w = Entry::new("3", "Wendy");
        w.status = EntryStatus::Withdrawn;
        ev.entries = vec![a, b, w];
        let rv = create_outright_view(&ev, &[]);
        let cars: Vec<&str> = rv.rows.keys().map(|c| c.as_str()).collect();
        assert_eq!(cars, vec!["1", "2"]);
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
    fn set_entry_status_updates_by_car() {
        let mut ev = EventInfo::default();
        ev.add_entry("7", "Alice");
        ev.add_entry("8", "Bob");
        assert!(ev.set_entry_status("7", EntryStatus::Accepted));
        assert!(ev.set_entry_status("8", EntryStatus::Withdrawn));
        assert!(!ev.set_entry_status("99", EntryStatus::Draft));
        let a = ev.entries.iter().find(|e| e.car == "7").unwrap();
        let b = ev.entries.iter().find(|e| e.car == "8").unwrap();
        assert_eq!(a.status, EntryStatus::Accepted);
        assert_eq!(b.status, EntryStatus::Withdrawn);
    }

    #[test]
    fn calc_pipeline_handles_multi_stage() {
        let ev = EventInfo {
            entries: vec![Entry::new("1", "Alice"), Entry::new("2", "Bob")],
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
            vec!["1".to_string(), "2".to_string()],
            "entries={:?} classes={:?}",
            ev.entries
                .iter()
                .map(|e| (&e.car, &e.classes))
                .collect::<Vec<_>>(),
            ev.classes
        );
        let alice = &rv.rows["1"].columns;
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
}
