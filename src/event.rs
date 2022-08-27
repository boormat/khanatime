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
    parse_display::Display,
    PartialEq,
    Debug,
    Serialize,
    Deserialize,
    Default,
    Clone,
)]

#[display("{time} {flags}F {garage}G")]
pub struct KTimeTime {
    pub time: f32,  pub flags: u8, pub  garage: bool, 
}

#[derive(
    // parse_display::FromStr,
    parse_display::Display,
    PartialEq,
    Debug,
    Serialize,
    Deserialize,
    Default,
    Clone,
)]


#[display("{}")]
pub enum KTime {
    #[default]
    NOSHO, // withdrawn, Did Not Start
    WD,
    FTS,
    DNF,
    TODO, // not run yet. Used in result calcs, and to render nice in results view
#[display("{0}")]
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
    // entries: u8, aka len rows
    // entries: Vec<Entry>, //todo use slice from context &'a [Entry];
    // rows: Vec<ResultRow<'a>>,
    rows: IndexMap<&'a str, ResultRow<'a>>, // list of know entrants/drivers. Ordered by car number
}

// results to render
#[derive(Debug)]
pub struct ResultRow<'a> {
    entry: &'a Entry, //todo use from context &'a [Entry];
    columns: Vec<ResultScore>,
    //cum_pos: Option<Pos>, // current/last cumulative position. None after a missed a stage
}

/// Results Position
///
#[derive(Default, Debug, Clone)]
pub struct Pos {
    score: f32,
    pos: u8,  // cumulative pos in event. Not unique for equal times
    eq: bool, // if pos is equal
}

// Result for a Driver in a Stage
#[derive(Default, Clone, Debug)]
pub struct ResultScore {
    // raw result fields
    flags: u8,
    garage: bool,
    time: KTime, // as entered.. maybe an enum? of codes and time? pritable, so time plus penalties etc.

    stage_pos: Pos, // result within stage
    cum_pos: Pos,   // pos in event.
    cum_change: i8, // indicator of changed event  position up/down from prev stage
}

impl<'a> ResultView<'a> {
    pub fn init(class: &str, event: &'a EventInfo) -> Self {
        //  entries: &'a [Entry]
        let entries = find_entries_in_class(&event.entries, class);

        let rows: IndexMap<&'a str, ResultRow<'a>> = 
        // let rows: Vec<ResultRow> = entries
            entries.iter()
            .map(|e| (&e.car[..], ResultRow::init(e, event.stages_count)))
            .collect();
        let class = class.to_string();

        Self { class, event, rows }
    }

    pub fn calc(&mut self) {
        // lets fill the grid, then use that.

        // alg tests? . hmmm

        // car to row/entry? Ordered HashMap?
        // for s in self.event.scores {
        //     s.car
        // }

        // Walk each stage doing the calcs
        for s in 0..self.event.stages_count {}
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
    }
}

impl<'a> ResultRow<'a> {
    pub fn init(entry: &'a Entry, stages: u8) -> Self {
        let columns: Vec<ResultScore> = vec![ResultScore::init(); stages as usize];
        Self { entry, columns }
    }
}

impl ResultScore {
    pub fn init() -> Self {
        let mut res = Self::default();
        res.time = KTime::TODO;
        res
    }
}

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

    let rv = ResultView::init(class, event);
    // rv.calc();
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


// get base times for a stage
// calc base. min  min*2 max
// pub fn calc_base_times<'a>(scores: &'a [&ScoreData], cars: &[&str], stages: u8) -> Vec<f32> {
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
// // get scores for the list of cars
// pub fn find_scores<'a>(scores: &'a [ScoreData], cars: &[&str]) -> Vec<&'a ScoreData> {
//     // &&s.car[..] ... uhhhh Ooookaaay
//     let a = scores
//         .iter()
//         .filter(|s| cars.contains(&&s.car[..]))
//         .collect();
//     a
// }
