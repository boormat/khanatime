use indexmap::{IndexMap, IndexSet};
use kts::khana_rule::RULES_MARKDOWN;
use seed::{prelude::*, *};
use serde::{Deserialize, Serialize};
use std::{
    cmp::max,
    collections::{HashMap, HashSet},
    mem,
};
// use uuid::Uuid;

const ENTER_KEY: u32 = 13;
const ESC_KEY: u32 = 27;
const UI_STORAGE_KEY: &str = "kts";
const EVENT_PREFIX: &str = "EVENT:";

// ------ ------
//     Init
// ------ ------

fn init(_: Url, _: &mut impl Orders<Msg>) -> Model {
    Model {
        events: list_events(),
        ui: SessionStorage::get(UI_STORAGE_KEY).unwrap_or_default(),
        event: Default::default(),
    }
}

// ------ ------
//     Models
// ------ ------

// ------ Model ------

struct Model {
    ui: CmdUi, // cmd prompt. Probably will want an enum to get a hint on what to do
    events: HashSet<String>, // names of known/stored events (local)
    event: Event, // active event
}

#[derive(Default, Serialize, Deserialize)]
struct CmdUi {
    // UI state.  Stored in session
    cmd: String, // cmd prompt. Probably will want an enum to get a hint on what to do
    page: Page,
    event: String, // curent event displayed
    stage: i8,     // curent stage displayed
}
#[derive(Default, Serialize, Deserialize)]
enum Page {
    #[default]
    Home,
    InStage,
    InEvent,
}

#[derive(Serialize, Deserialize)]
struct Event {
    name: String,
    stages_count: i8, // number of stages to run. 1 indexed
    // stages: HashSet<i8>, // stage numbers/index (we might have to skip some?)
    // times: Vec<RawScore>,                            // raw times, order of insertion
    scores: HashMap<i8, HashMap<String, CalcScore>>, // calculated for display.  Key is [stage][car] holding a Score.
    classes: IndexSet<String>,                       // list of known classes. Order as per display
    entries: IndexMap<String, Entry>, // list of know entrants/drivers. Ordered by car number
}

#[derive(Default, Serialize, Deserialize)]
struct CalcScore {
    // keys
    car: String,
    stage: i8,

    // date
    time: Time, // as entered.. maybe an enum? of codes and time? pritable, so time plus penalties etc.
    flags: i8,

    // calculated
    score: f32, // derived time
    // positions.
    pos_stage: Pos,
    pos_outright: Pos,
}

#[derive(Default, Serialize, Deserialize, PartialEq, Eq)]
struct Pos {
    order: i8,                    // overall pos, for sorting.. might not be required?
    pos: HashMap<String, String>, // columname/Classname vs position. Posn is String for =2nd and suchlike
}

#[derive(Default, Serialize, Deserialize)]
enum Time {
    #[default]
    DNS,
    WD,
    FTS,
    DNF,
    Time(f32), // seconds
}
#[derive(Default, Serialize, Deserialize, PartialEq, Eq)]
struct Entry {
    car: String,     // entry/car number
    name: String,    // name
    vehicle: String, // description
    classes: HashSet<String>, // Classname vs position.
                     // Display sorting maintained in event/File
                     // order: f32, // sort order based on car oe.g. '0A', '00'.  User can edit, eg  handle seeding
}

impl Default for Event {
    fn default() -> Self {
        // let letters: IndexSet<_> = "a short treatise on fungi".chars().collect();

        // let a: Vec<_> = vec!["Outright", "Female", "Junior"];
        // a.t .map(|a|) {a. to_string};
        // let b: IndexSet<_> = a.iter().collect(); // ndexSet::from_iter(a);
        // let d = b;
        // let classes = IndexSet::from_iter(default_classes());
        Self {
            // name: "Event TBA2".to_owned(),
            // stages
            name: Default::default(),
            // stages: Default::default(),
            stages_count: 10, // default is 10
            // times: Default::default(),
            scores: Default::default(),
            entries: Default::default(),
            classes: IndexSet::from_iter(default_classes()),
        }
    }
}

fn default_classes() -> Vec<String> {
    ["Outright", "Female", "Junior"]
        .iter()
        .map(|&s| s.into())
        .collect::<_>()
}

// impl Default for Time {
//     fn default() -> Self {
//         Time::DNS
//     }
// }
// need Class eventauuily.  Driver/car + outright.  Or just filter in/out.
// Probably like spreadsheet .. posn vs class.
// entrant gets all the classes y/n
// calc posn of relevant class.
// #[derive(Default, Serialize, Deserialize)]
// ------ ------
//    Update
// ------ ------

enum Msg {
    DataEntry(String),
    CreateEvent,
    CancelEdit,
    ShowStage,
    AddTime,
}

fn update(msg: Msg, model: &mut Model, _orders: &mut impl Orders<Msg>) {
    match msg {
        // text box typing
        Msg::DataEntry(value) => {
            model.ui.cmd = value;
        }
        Msg::CancelEdit => {
            model.ui.cmd.clear();
        }
        Msg::CreateEvent => {
            model.event.name = mem::take(&mut model.ui.cmd);
            model.ui.page = Page::InEvent;
        }
        Msg::ShowStage => {
            // creates it if new.  Cmd is number space optional name
            // switch  (no, name) = model.cmd.cmd.split_once(" "){
            // ?;// whitespace();
            // hmm need validation!
            // model.event.stages. = mem::take(&mut model.cmd.cmd);
            if let Ok(i) = model.ui.cmd.parse() {
                model.ui.page = Page::InStage;
                model.ui.stage = i; // type from here. Sheesh turbofish ::<
                model.event.stages_count = max(i, model.event.stages_count);
            }; //; = model.cmd.cmd.to
        }
        Msg::AddTime => todo!(),
    }
    // Note: It should be optimized in a real-world application.
    // LocalStorage::insert(UI_STORAGE_KEY, &model.cmd)
    //     .expect("save UI stage to LocalStorage ... Session variables");
    if !model.event.name.is_empty() {
        let key = format!("{}{}", EVENT_PREFIX, model.event.name);
        LocalStorage::insert(key, &model.event).expect("save data to LocalStorage");
    }
}

/// list of known events in storage.  String is storage key, is the event name
/// if it fails .. empty is fine
fn list_events() -> HashSet<String> {
    let len = LocalStorage::len().unwrap_or_default();
    let mut out: HashSet<String> = Default::default();
    // ugly it up with map?
    // out.push("dog".to_string());
    (0..len).for_each(|i| {
        if let Ok(name) = LocalStorage::key(i) {
            if name.starts_with(EVENT_PREFIX) {
                out.insert(name[EVENT_PREFIX.len()..].to_string());
            }
        }
    });
    return out;
}

// ------ ------
//     View
// ------ ------

fn view(model: &Model) -> impl IntoNodes<Msg> {
    // let data = &model.data;
    match model.ui.page {
        Page::Home => view_no_event(&model),
        Page::InEvent => view_show_event(&model),
        Page::InStage => view_show_stage(&model),
    }
    // nodes![
    //     vec![
    //         view_header(&model.event.name),
    //         view_main(
    //             &data.todos,
    //             data.filter,
    //             &data.editing_todo,
    //             &model.refs.editing_todo_input,
    //         ),
    //         view_footer(&data.todos, data.filter),
    //         view_event(&model.event),
    //     ]
    // },]
}

fn view_rules() -> Vec<Node<Msg>> {
    Node::from_markdown(RULES_MARKDOWN)
}

// ------ header ------
fn view_no_event(model: &Model) -> Node<Msg> {
    // let var`
    header![
        // C!["header"],
        h1!["KhanaTimingSystem"],
        input![
            C!["new-todo"],
            attrs! {
                At::Placeholder => "New Event Name?"; // this changes
                At::AutoFocus => true.as_at_value();
                At::Value => model.ui.cmd;
            },
            keyboard_ev(Ev::KeyDown, |keyboard_event| {
                match keyboard_event.key_code() {
                    ENTER_KEY => Some(Msg::CreateEvent),
                    ESC_KEY => Some(Msg::CancelEdit),
                    _ => None,
                }
            }),
            input_ev(Ev::Input, Msg::DataEntry),
        ],
        view_event_links(&model),
        view_rules(),
    ]
}

fn view_show_event(model: &Model) -> Node<Msg> {
    header![
        C!["header"],
        h1![format! {"KTS: {}" , model.event.name}],
        view_stage_links(model),
        input![
            C!["new-todo"],
            attrs! {
                At::Placeholder => "stage to edit?"; // this changes
                At::AutoFocus => true.as_at_value();
                At::Value => model.ui.cmd;
            },
            keyboard_ev(Ev::KeyDown, |keyboard_event| {
                match keyboard_event.key_code() {
                    ENTER_KEY => Some(Msg::ShowStage),
                    ESC_KEY => Some(Msg::CancelEdit),
                    _ => None,
                }
            }),
            input_ev(Ev::Input, Msg::DataEntry),
        ]
    ]
}

fn view_show_stage(model: &Model) -> Node<Msg> {
    header![
        C!["header"],
        h1![format! {"IN thingy {}" , model.event.name}],
        h1![format! {"KTS: {}" , model.event.name}],
        view_stage_links(model),
        input![
            C!["new-todo"],
            attrs! {
                At::Placeholder => "stage to edit?"; // this changes
                At::AutoFocus => true.as_at_value();
                At::Value => model.ui.cmd;
            },
            keyboard_ev(Ev::KeyDown, |keyboard_event| {
                match keyboard_event.key_code() {
                    ENTER_KEY => Some(Msg::AddTime),
                    ESC_KEY => Some(Msg::CancelEdit),
                    _ => None,
                }
            }),
            input_ev(Ev::Input, Msg::DataEntry),
        ]
    ]
}

fn view_event_links(model: &Model) -> Node<Msg> {
    ul![
        C!["events"],
        model.events.iter().map(|name| {
            // let current = *stage == model.cmd.stage;
            view_event_link(&name)
        })
    ]
}
fn view_event_link(name: &String) -> Node<Msg> {
    li![a![
        attrs! {
            At::Href => format!("/{}", name)
        },
        style! {St::Cursor => "pointer"},
        format!("{}", name)
    ]]
}

fn view_stage_links(model: &Model) -> Node<Msg> {
    ul![
        C!["stages"],
        (1..model.event.stages_count).map(|stage| {
            let current = stage == model.ui.stage;
            view_stage_link(stage, current)
        })
    ]
}

fn view_stage_link(stage: i8, selected: bool) -> Node<Msg> {
    li![a![
        C![IF!(selected => "selected")],
        attrs! {
            At::Href => format!("/{}", stage)
        },
        style! {St::Cursor => "pointer"},
        format!("{}", stage)
    ]]
}

// ------ ------
//     Start
// ------ ------

fn main() {
    App::start("app", init, update, view);
}
