// Structure for in memory storage of event
// probably will do serialisation for long term storage

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

// Inputs raw scores.  From scores page.
// Entry list.  Name Vs Class.
// Class
// maybe a sort?  (Might be best later, pick a column, car, driver, score per stage)
pub fn create_result_view(event: &EventInfo, class: &str) -> ResultView {
    // Calc min time per stage (for class)
    // loop raw results... list of cars eligible.  Find relevant results.
    // sort into stages.

    // validate ? Complain about scores for non-existant cars
    // times for non-existant stages

    let cars = find_cars_in_class(&event.entries, class);
    let scores = find_scores(&event.scores, &cars[..]);

    // base times, per stage...
    let bases = base_times(&scores[..], &cars[..], event.stages_count);

    // fill vec of Result Scores. per car
    for car in cars {
        // let columns: Vec<ResultScore>,
        let row = car_scores(&bases[..], &scores[..], car);
        // let bases = base_times(&scores[..], &cars[..], event.stages_count);
    }

    Default::default()
    // !todo();
}

// calculate list of stage scores (columns) for the car
pub fn car_scores<'a>(
    base_times: &[f32],
    scores: &'a [&ScoreData],
    cars: &str,
) -> Vec<ResultScore> {
    // need helper to ctor a ResultScore from ScoreData, pass in base_time.
    // IF not found ensure default is DNS I guess.
    // this one does a search for the score?
    Default::default()
}
// get base times for each stage
// calc base. min  min*2 max
pub fn base_times<'a>(scores: &'a [&ScoreData], cars: &[&str], stages: u8) -> Vec<f32> {
    let mut min = vec![0f32; stages.into()];
    let mut max = vec![0f32; stages.into()];

    for s in scores.iter() {
        if let KTime::Time(time) = s.time {
            // will panic if stage is out of range. meh
            let stage: usize = s.stage.into();
            min[stage] = min[stage].min(time);
            max[stage] = max[stage].max(time);
        }
    }

    let out = std::iter::zip(min, max)
        .map(|(min, max)| max.min(2f32 * min))
        .collect();
    out
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

// get scores for the list of cars
pub fn find_scores<'a>(scores: &'a [ScoreData], cars: &[&str]) -> Vec<&'a ScoreData> {
    // &&s.car[..] ... uhhhh Ooookaaay
    let a = scores
        .iter()
        .filter(|s| cars.contains(&&s.car[..]))
        .collect();
    a
}
// Event INFO.  Staticish
#[derive(Serialize, Deserialize)]
pub struct EventInfo {
    name: String,
    stages_count: u8, // number of stages planned to run. 1 indexed

    // scores: HashMap<i8, HashMap<String, CalcScore>>, // calculated for display.  Key is [stage][car] holding a Score.
    classes: Vec<String>, // list of known classes. Order as per display
    entries: Vec<Entry>,  // list of know entrants/drivers. Ordered by something

    scores: Vec<ScoreData>, // the raw score log
}

#[derive(Default, Serialize, Deserialize, PartialEq, Eq)]
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
    pub flags: u8,
    pub garage: u8,
}

// #[derive(Copy, Clone, Default, Deserialize, PartialEq, Debug)]
#[derive(
    parse_display::FromStr,
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
    NOSHO,
    WD,
    FTS,
    DNF,
    #[display("{0}")]
    Time(f32),
}
// NOTE Result ordering CAN change for classes.
// Maybe we should have a Display Score focussing on class? ie. regen after filter
// is selected.
// results to render
#[derive(Default, Debug)]
pub struct ResultView {
    class: String,
    event: String,
    rows: Vec<ResultRow>,
}

// results to render
#[derive(Default, Debug)]
pub struct ResultRow {
    car: String, // car number 0A 1 2 3
    columns: Vec<ResultScore>,
    name: String,    // name
    vehicle: String, // description
}

// A Single score for a stage/column
#[derive(Default, Debug)]
pub struct ResultScore {
    // info to display
    flags: u8,
    garage: bool,
    time: KTime, // as entered.. maybe an enum? of codes and time? pritable, so time plus penalties etc.
    score: f32,  //calculated time for THIS class
    stage_pos: String, // name of pos in stage Posn is String for =2nd and suchlike
    event_pos: String, // name of pos in event at this stage.
    change: i8,  // indicator of changed event  position up/down from prev stage
}

// #[derive(Default, Debug)]
// pub enum Time {
//     #[default]
//     DNS,
//     WD,
//     FTS,
//     DNF,
//     Time(f32), // seconds
// }
