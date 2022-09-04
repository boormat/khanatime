use crate::event::EventInfo;
use crate::event::KTime;
use crate::event::ResultView;
use crate::event::ScoreData;

// Results view.
// Render ResultView
// Model to sorting
// Change Class
use seed::{prelude::*, *};

// #[derive(Serialize, Deserialize, Clone)]
pub enum Msg {
    SortStage,
    SortEvent,
    SortDriver,
    ShowClass(String),
}

pub struct Model {
    results: Option<ResultView>,
    event: Option<EventInfo>,
}

pub fn init() -> Model {
    let mut model = Model {
        results: None,
        event: None,
    };
    load_ui(&mut model);
    load_event(&mut model);
    model
}

const STAGEPAGE_PREFIX: &str = "stagepage:";
// const STAGEPAGE_PREFIX: &str = "eventstagepage:";

fn load_event(model: &mut Model) {
    // if !model.event.is_empty() {
    //     let key = format!("{}{}", STAGEPAGE_PREFIX, model.event);
    //     let s = LocalStorage::get(&key).unwrap_or_default();
    //     model.scores = s;
    // }
}

fn load_ui(model: &mut Model) {
    if let Ok(event) = SessionStorage::get("event") {
        model.event = event;
    }
}

fn save_ui(model: &Model) {
    SessionStorage::insert("event", &model.event).expect("save data to SessionStorage");
}

pub fn update(msg: Msg, model: &mut Model) {
    match msg {
        Msg::SortStage => todo!(),
        Msg::SortEvent => todo!(),
        Msg::SortDriver => todo!(),
        Msg::ShowClass(_) => todo!(),
    }
}

pub fn view(model: &Model) -> Node<Msg> {
    div! {
        // h1![format!("Event: {} Stage:{}", model.event, model.stage)],
        // sort buttons.
        // results list... here
        view_list(&model),
    }
}

fn view_list(model: &Model) -> Node<Msg> {
    let mut v = vec![view_time_header()];
    // for a in model.scores.iter() {
    //     v.push(view_time(&a));
    // }
    table![v]
}

fn view_time_header() -> Node<Msg> {
    tr![th!["Stage"], th!["Car"], th!["Time"], th!["Flags"],]
}
fn view_time(score: &ScoreData) -> Node<Msg> {
    tr![
        td![score.stage.to_string()],
        td![view_car_number(&score.car)],
        td![view_time_score(&score.time)],
    ]
}

fn view_time_score(time: &KTime) -> Node<Msg> {
    log!(time.to_string());
    div!(time.to_string())
}

fn view_car_number(car: &String) -> Node<Msg> {
    span! {
        C!["label label-default"],
        car
    }
}
