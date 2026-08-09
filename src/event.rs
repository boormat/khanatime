// Structure for in memory storage of event
// probably will do serialisation for long term storage

use std::collections::HashSet;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

// Event INFO.  Staticish
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EventInfo {
    pub name: String,
    pub stages_count: u8, // number of stages planned to run. 1 indexed

    // scores: HashMap<i8, HashMap<String, CalcScore>>, // calculated for display.  Key is [stage][car] holding a Score.
    pub classes: Vec<String>, // list of known classes. Order as per display
    pub entries: Vec<Entry>,  // list of know entrants/drivers. Ordered by something
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Entry {
    pub car: String,          // entry/car number
    pub name: String,         // name
    pub vehicle: String,      // description
    pub classes: Vec<String>, // Classes. Count be an ID. meh
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
        let name = "TBA".into();
        let stages_count = 12;
        let entries = vec![];
        Self {
            name,
            stages_count,
            classes,
            entries,
            // scores,
        }
    }
}

impl EventInfo {
    pub fn add_class(&mut self, class: &String) {
        if self.classes.contains(class) {
            return;
        }
        self.classes.push(class.clone());
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

        let base_times_ds = vec![0; event.stages_count as usize];
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
        let columns = (0..event.stages_count)
            .map(|col| find_score(scores, &entry.car[..], col + 1).map(ResultScore::init))
            .collect();

        Self {
            entry: entry.clone(),
            columns,
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
    for stage in 0..rv.event.stages_count {
        let mut fastest: u32 = u16::MAX as u32;
        let mut slowest: u32 = 0;
        for row in rv.rows.values() {
            if let Some(ResultScore {
                time: KTime::Time(kt),
                ..
            }) = &row.columns[stage as usize]
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
        rv.base_times_ds[stage as usize] = base_time as u16;
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
    for stage in 0..rv.event.stages_count {
        for row in rv.rows.values_mut() {
            let base_time = rv.base_times_ds[stage as usize];
            let plus10 = base_time + 100;
            let plus5 = base_time + 50;

            if let Some(rs) = &mut row.columns[stage as usize] {
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
        let mut stage = 0;
        while let Some(rs) = &mut row.columns[stage as usize] {
            score += rs.stage_pos.score_ds;
            rs.cum_pos = Some(Pos::init(score));
            stage += 1;
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
        let mut stage = 0usize;
        while let Some(cum_pos) = get_cum_pos(&mut row.columns[stage]) {
            if stage > 0 {
                // show no change in col 1?
                cum_pos.change = last_rank as i8 - cum_pos.pos as i8;
            }
            last_rank = cum_pos.pos;
            stage += 1;
        }
    }
}

pub fn calc_stage_positions(rv: &mut ResultView) {
    for stage in 0..rv.event.stages_count {
        // collect pairs, rowkey (car) vs time
        // could collect a mut pos & too?
        let mut car_scores: Vec<(&str, &mut Pos)> = vec![];
        for (rowkey, rr) in rv.rows.iter_mut() {
            if let Some(rs) = &mut rr.columns[stage as usize] {
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
    for stage in 0..rv.event.stages_count {
        // collect pairs, rowkey (car) vs time
        // could collect a mut pos & too?
        let mut car_scores: Vec<(&str, &mut Pos)> = vec![];
        for (rowkey, rr) in rv.rows.iter_mut() {
            if let Some(rs) = &mut rr.columns[stage as usize] {
                if let Some(cum_pos) = &mut rs.cum_pos {
                    car_scores.push((rowkey.as_str(), cum_pos));
                }
            }
        }
        calc_rank(&mut car_scores);
    }
}

pub fn calc(rv: &mut ResultView) {
    calc_base_times(rv);
    calc_penalties(rv);
    calc_stage_positions(rv);
    calc_cumulative_times(rv);
    calc_cumulative_positions(rv);
    calc_pos_changes(rv);
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

// get entries  in class
pub fn find_entries_in_class<'a>(entries: &'a [Entry], class: &str) -> Vec<&'a Entry> {
    //    return vec![&scores[0]];
    let class = class.to_string();
    let a = entries
        .iter()
        .filter(|e| e.classes.contains(&class))
        .collect();
    a
}

// get available Raw scores for the list of cars in a stage
pub fn find_score<'a>(scores: &'a [ScoreData], car: &str, stage: u8) -> Option<&'a ScoreData> {
    scores.iter().find(|s| s.stage == stage && car == s.car)
}

const EVENT_PREFIX: &str = "event:";
const TIMES_PREFIX: &str = "times:";
const EVENT_SESSION: &str = "event";

fn event_key(name: &String) -> String {
    format!("{}{}", EVENT_PREFIX, name)
}

fn times_key(name: &String) -> String {
    format!("{}{}", TIMES_PREFIX, name)
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

pub fn load_event(name: &String) -> EventInfo {
    if !name.is_empty() {
        let key = event_key(name);
        let mut e: EventInfo = get_json(&key).unwrap_or_default();
        e.name = name.to_string(); // change if default, just fix
        e
    } else {
        EventInfo {
            name: name.to_string(),
            ..Default::default()
        }
    }
}

pub fn save_event(event: &EventInfo) {
    let key = event_key(&event.name);
    set_json(&key, event);
}

/// list of known events in storage.  String is storage key, is the event name
/// if it fails .. empty is fine
pub fn list_events() -> HashSet<String> {
    let mut out: HashSet<String> = Default::default();
    if let Some(st) = storage() {
        if let Ok(len) = st.length() {
            (0..len).for_each(|i| {
                if let Ok(Some(name)) = st.key(i) {
                    if let Some(rest) = name.strip_prefix(EVENT_PREFIX) {
                        out.insert(rest.to_string());
                    }
                }
            });
        }
    }
    out
}

pub fn load_times(name: &String) -> Vec<ScoreData> {
    if !name.is_empty() {
        let key = times_key(name);
        get_json(&key).unwrap_or_default()
    } else {
        vec![]
    }
}

pub fn save_times(name: &String, scores: &Vec<ScoreData>) {
    if !name.is_empty() {
        let key = times_key(name);
        set_json(&key, scores);
    }
}

pub fn session_event_name() -> String {
    session_storage()
        .and_then(|st| st.get_item(EVENT_SESSION).ok().flatten())
        .unwrap_or_else(|| "TBA".to_string())
}

pub fn session_set_event(name: &str) {
    if let Some(st) = session_storage() {
        let _ = st.set_item(EVENT_SESSION, name);
    }
}
