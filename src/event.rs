// Structure for in memory storage of event
// probably will do serialisation for long term storage

use indexmap::IndexMap;
// use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};

// #[derive(Serialize, Deserialize, Debug)]
// struct Event {
//     name: String,
//     stages_count: u8, // number of stages to run. 1 indexed
//     // stages: HashSet<i8>, // stage numbers/index (we might have to skip some?)
//     // times: Vec<RawScore>,                            // raw times, order of insertion
//     scores: HashMap<u8, HashMap<String, CalcScore>>, // calculated for display.  Key is [stage][car] holding a Score.
//     classes: IndexSet<String>,                       // list of known classes. Order as per display
//     entries: IndexMap<String, Entry>, // list of know entrants/drivers. Ordered by car number
//                                       // 'base' times per class per stage.
// }

// Event INFO.  Staticish
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct EventInfo {
    name: String,
    stages_count: u8, // number of stages planned to run. 1 indexed

    // scores: HashMap<i8, HashMap<String, CalcScore>>, // calculated for display.  Key is [stage][car] holding a Score.
    classes: Vec<String>, // list of known classes. Order as per display
    entries: Vec<Entry>,  // list of know entrants/drivers. Ordered by something

    scores: Vec<ScoreData>, // the raw score log
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    car: String,          // entry/car number
    name: String,         // name
    vehicle: String,      // description
    classes: Vec<String>, // Classes. Count be an ID. meh
}

#[derive(Default, Serialize, Deserialize, Debug)]
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
#[derive(Debug)]
pub struct ResultView<'a> {
    event: &'a EventInfo,
    class: String,
    rows: IndexMap<&'a str, ResultRow<'a>>, // list of know entrants/drivers. Ordered by car number
    base_times_ds: Vec<u16>,                // base times
}

// results to render
#[derive(Debug)]
pub struct ResultRow<'a> {
    entry: &'a Entry, //todo use from context &'a [Entry];
    columns: Vec<Option<ResultScore>>,
    //cum_pos: Option<Pos>, // current/last cumulative position. None after a missed a stage
}

/// Results Position
///
#[derive(Default, Debug, Clone)]
pub struct Pos {
    score_ds: u16, // time in ds, after penalites
    pos: u8,       // cumulative pos in event. Not unique for equal times
    eq: bool,      // if pos is equal
    change: u8,    // delta of last stage (cumulative only?)
}

// Result for a Driver in a Stage
#[derive(Default, Clone, Debug)]
pub struct ResultScore {
    // raw result fields
    time: KTime, // as entered.. maybe an enum? of codes and time? pritable, so time plus penalties etc.
    stage_pos: Pos, // result within stage
    cum_pos: Option<Pos>, // pos in event.
}

impl<'a> ResultView<'a> {
    pub fn init(class: &str, event: &'a EventInfo) -> Self {
        //  entries: &'a [Entry]
        let entries = find_entries_in_class(&event.entries, class);

        // let rows: Vec<ResultRow> = entries
        let rows: IndexMap<&'a str, ResultRow<'a>> = entries
            .iter()
            .map(|e| (&e.car[..], ResultRow::init(e, event)))
            .collect();
        let class = class.to_string();

        // let base_times = calc_base_times(event);
        let base_times_ds = vec![0; event.stages_count as usize];
        Self {
            class,
            event,
            rows,
            base_times_ds,
        }
    }
}

impl<'a> ResultRow<'a> {
    pub fn init(entry: &'a Entry, event: &'a EventInfo) -> Self {
        let columns = (0..event.stages_count)
            .map(
                |stage| match find_score(&event.scores[..], &entry.car[..], stage) {
                    None => None,
                    Some(rs) => Some(ResultScore::init(rs)),
                },
            )
            .collect();

        Self { entry, columns }
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
        let mut min = 0;
        let mut max = 0;
        for row in rv.rows.values() {
            match &row.columns[stage as usize] {
                Some(ResultScore {
                    time: KTime::Time(kt),
                    ..
                }) => {
                    // if let KTime::Time(time) = rs{
                    if kt.garage as u8 + kt.flags == 0 {
                        min = min.min(kt.time_ds);
                        max = max.max(kt.time_ds);
                    }
                }
                _ => {}
            }
            let base_time = max.min(2 * min);
            rv.base_times_ds[stage as usize] = base_time;
        }
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
pub fn calc_stage_positions(rv: &mut ResultView) {
    for stage in 0..rv.event.stages_count {
        // collect pairs, rowkey (car) vs time
        let mut car_scores = vec![];
        for (rowkey, rr) in rv.rows.iter() {
            if let Some(rs) = &rr.columns[stage as usize] {
                car_scores.push((*rowkey, rs.stage_pos.score_ds));
            }
        }

        // sort by score
        car_scores.sort_unstable_by_key(|a| a.1);

        // calc the ranks and eq
        // (key, score, rank, eq)
        let mut last_time = 0u16;
        let mut rank = 1u8;
        let ranked: Vec<(&str, u16, u8, bool)> = car_scores
            .iter()
            .enumerate()
            .map(|(idx, (rowkey, score))| {
                let eq = *score == last_time;
                last_time = *score;
                if !eq {
                    rank = idx as u8 + 1
                };
                (*rowkey, *score, rank, eq)
            })
            .collect();

        // poke stage results back in rows
        for (rowkey, score, rank, eq) in ranked.iter() {
            let row = &mut rv.rows[rowkey];
            let colo = &mut row.columns[stage as usize];
            let rs = &mut colo.as_mut().unwrap();
            // let a: &mut ResultScore = &mut rv.rows[cs.0].columns[stage as usize].as_mut().unwrap();
            rs.stage_pos.pos = *rank;
            rs.stage_pos.eq = *eq;
            rs.stage_pos.score_ds = *score; // not sure need this.
        }
        // car_scores.sort_unstable_by_key(|(_key, &val)| val);
    }
}

pub fn calc(rv: &mut ResultView) {
    calc_base_times(rv);
    calc_penalties(rv);
    calc_stage_positions(rv);
    // calc cumulative times
    // calc cum positions
}
// Walk each stage doing the calcs
// for s in 0..rv.event.stages_count {
//     rv.rows

// }
//     let raw_scores: Vec<&ScoreData> = find_scores(&event.scores, &cars[..]);
//     let mut stage_res: Vec<Option<ResultScore>> = vec![None; cars.len()];

//     // let sz: usize = s.into();
//     let mut s_times = vec![0f32; cars.len()]; // cumulative time for calcs
//     let mut s_scores = vec![KTime::NOSHO; cars.len()]; // cumulative time for calcs
//     for caridx in 0..cars.len() {
//         let car = cars[caridx];
//         (s_times[caridx], s_scores[caridx]) =
//             get_car_stage_score(&raw_scores[..], base_times[s as usize], car, s);
//         cum_time[caridx] += time[caridx];
//     }

// now calc pos in stage
// much simpler if use a struct or tuple so can sort easy
// can prolly do the string label <= etc in render?  otherwise needs sort
// then markup
// }

// let columns: Vec<ResultScore>,
// let row = car_scores(&bases[..], &scores[..], car);
// let bases = base_times(&scores[..], &cars[..], event.stages_count);

// Inputs raw scores.  From scores page.
// Entry list.  Name Vs Class.
// Class
// maybe a sort?  (Might be best later, pick a column, car, driver, score per stage)
pub fn create_result_view<'a>(event: &'a EventInfo, class: &str) -> ResultView<'a> {
    // Calc min time per stage (for class)
    // loop raw results... list of cars eligible.  Find relevant results.
    // sort into stages.

    // validate ? Complain about scores for non-existant cars
    // times for non-existant stages

    let mut rv = ResultView::init(class, event);
    calc(&mut rv);
    rv
}

// calculate the score for a car in stage.  Return pair of score and code
// marks car as dnf if no result. (REALLY should do that earlier and save pain?)
// pub fn get_car_stage_score(
//     scores: &[&ScoreData],
//     base_time: f32,
//     car: &str,
//     stage: u8,
// ) -> (f32, KTime) {
//     let no_sho_time = base_time + 10f32;
//     let wd_time = base_time + 5f32;

//     match scores.iter().find(|s| s.car == car && s.stage == stage) {
//         None => (no_sho_time, KTime::NOSHO),
//         Some(s) => {
//             let score = match s.time {
//                 KTime::NOSHO => no_sho_time,
//                 KTime::WD => wd_time,
//                 KTime::FTS => wd_time,
//                 KTime::DNF => wd_time,
//                 KTime::Time(t) => t + (5 * (s.flags + s.garage)) as f32,
//                 KTime::TODO => base_time,
//             };
//             (score, s.time)
//         }
//     }
// }

// // get base times for a stage
// // calc base. min  min*2 max
// pub fn calc_base_times<'a>(escores: &'a [&ScoreData], cars: &[&str], stages: u8) -> Vec<f32> {
//     let mut min = vec![0f32; stages.into()];
//     let mut max = vec![0f32; stages.into()];

//     for s in scores.iter() {
//         if let KTime::Time(time) = s.time {
//             if (time.garage as u8 + time.flags) == 0 {
//             // will panic if stage is out of range. meh
//             let stage: usize = s.stage.into();
//             min[stage] = min[stage].min(time.time);
//             max[stage] = max[stage].max(time.time);
//             }
//         }
//     }

//     let out = std::iter::zip(min, max)
//         .map(|(min, max)| max.min(2f32 * min))
//         .collect();
//     out
// }

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

// get car #s in class
pub fn find_cars_in_class<'a>(entries: &'a [Entry], class: &str) -> Vec<&'a str> {
    //    return vec![&scores[0]];
    let class = class.to_string();
    let a = entries
        .iter()
        .filter(|e| e.classes.contains(&class))
        .map(|e| &e.car[..])
        .collect();
    a
}

// get available Raw scores for the list of cars in a stage
pub fn find_scores<'a>(scores: &'a [ScoreData], cars: &[&str], stage: u8) -> Vec<&'a ScoreData> {
    // &&s.car[..] ... uhhhh Ooookaaay
    let a = scores
        .iter()
        .filter(|s| s.stage == stage && cars.contains(&&s.car[..]))
        .collect();
    a
}

// get available Raw scores for the list of cars in a stage
pub fn find_score<'a>(scores: &'a [ScoreData], car: &str, stage: u8) -> Option<&'a ScoreData> {
    scores.iter().find(|s| s.stage == stage && car == s.car)
}

// // get scores for the list of cars
// pub fn find_scores<'a>(scores: &'a [ScoreData], cars: &[&str]) -> Vec<&'a ScoreData> {
//     // &&s.car[..] ... uhhhh Ooookaaay
//     let a = scores
//         .iter()
//         .filter(|s| cars.contains(&&s.car[..]))
//         .collect();
//     a
// }
