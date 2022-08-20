mod page;

use indexmap::{IndexMap, IndexSet};
use seed::{prelude::*, *};
use serde::{Deserialize, Serialize};
use std::{
    cmp::max,
    collections::{HashMap, HashSet},
    mem,
};

const ENTER_KEY: u32 = 13;
const ESC_KEY: u32 = 27;
const UI_STORAGE_KEY: &str = "kts";
const EVENT_PREFIX: &str = "EVENT:";

fn init(_: Url, _: &mut impl Orders<Msg>) -> Model {
    Model {
        ui: SessionStorage::get(UI_STORAGE_KEY).unwrap_or_default(),
        page: Page::Home,
        events: list_events(),
        event: Default::default(),
        ctx: Default::default(),
        stage_model: page::stage::init(),
    }
}

struct Model {
    ui: CmdUi, // cmd prompt. Probably will want an enum to get a hint on what to do
    ctx: Context,
    page: Page,
    #[allow(dead_code)]
    events: HashSet<String>, // names of known/stored events (local)
    event: Event, // active event
    stage_model: page::stage::StageModel,
}

#[derive(Default)]
struct Context {
    user: Option<User>,
}
#[derive(Deserialize)]
struct User {
    // name: String,
}
#[derive(Default, Serialize, Deserialize)]
struct CmdUi {
    // UI state.  Stored in session
    cmd: String,   // cmd prompt. Probably will want an enum to get a hint on what to do
    event: String, // curent event displayed
    stage: i8,     // curent stage displayed
}
#[derive(Default, Serialize, Deserialize)]
pub enum Page {
    #[default]
    Home,
    KhanaRules,
    NotFound,
    Stage,
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

pub enum Msg {
    DataEntry(String),
    CreateEvent,
    CancelEdit,
    ShowStage,
    Show(Page),
    StageMsg(page::stage::StageMsg),
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
            model.page = Page::InEvent;
        }
        Msg::ShowStage => {
            // creates it if new.  Cmd is number space optional name
            // switch  (no, name) = model.cmd.cmd.split_once(" "){
            // ?;// whitespace();
            // hmm need validation!
            // model.event.stages. = mem::take(&mut model.cmd.cmd);
            if let Ok(i) = model.ui.cmd.parse() {
                model.page = Page::Stage;
                model.ui.stage = i; // type from here. Sheesh turbofish ::<
                model.event.stages_count = max(i, model.event.stages_count);
            }; //; = model.cmd.cmd.to
        }
        Msg::Show(p) => model.page = p,
        Msg::StageMsg(stage_msg) => page::stage::update(stage_msg, &mut model.stage_model),
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

fn view(model: &Model) -> Vec<Node<Msg>> {
    vec![
        view_navbar(model.ctx.user.as_ref(), &model.page),
        view_content(&model),
    ]
}

// ----- view_content ------

fn view_content(model: &Model) -> Node<Msg> {
    div![
        C!["container"],
        match model.page {
            Page::Home => page::home::view(),
            Page::KhanaRules => page::khana_rule::view(),
            Page::NotFound => page::not_found::view(),
            Page::InEvent => span!("Oops"), //view_show_event(&model),
            Page::Stage => {
                page::stage::view(&model.stage_model).map_msg(Msg::StageMsg)
            }
        }
    ]
}

fn view_navbar(_user: Option<&User>, page: &Page) -> Node<Msg> {
    nav![
        C!["navbar", "is-link"],
        attrs! {
            At::from("role") => "navigation",
            At::AriaLabel => "main navigation",
        },
        div![
            C!["navbar-brand"],
            a![
                linky2(matches!(page, Page::Home)),
                "KTS",
                ev(Ev::Click, |_| Msg::Show(Page::Home)),
            ],
            a![
                linky2(matches!(page, Page::KhanaRules)),
                "Rules",
                ev(Ev::Click, |_| Msg::Show(Page::KhanaRules)),
            ],
            a![
                linky2(matches!(page, Page::Stage)),
                "Stages",
                ev(Ev::Click, |_| Msg::Show(Page::Stage)),
            ],
        ]
    ]
}

fn linky2(active: bool) -> Attrs {
    C![
        "navbar-item",
        "has-text-weight-bold",
        "is-size-5",
        IF!(active => "is-active"),
    ]
}

// ------ header ------
#[allow(dead_code)]
fn view_no_event(model: &Model) -> Node<Msg> {
    header![
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
    ]
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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
                    // ENTER_KEY => Some(Msg::AddTime),
                    ESC_KEY => Some(Msg::CancelEdit),
                    _ => None,
                }
            }),
            input_ev(Ev::Input, Msg::DataEntry),
        ]
    ]
}

#[allow(dead_code)]
fn view_event_links(model: &Model) -> Node<Msg> {
    ul![
        C!["events"],
        model.events.iter().map(|name| { view_event_link(&name) })
    ]
}

#[allow(dead_code)]
fn view_event_link(name: &String) -> Node<Msg> {
    li![a![
        attrs! {
            At::Href => format!("/{}", name)
        },
        style! {St::Cursor => "pointer"},
        format!("{}", name)
    ]]
}

#[allow(dead_code)]
fn view_stage_links(model: &Model) -> Node<Msg> {
    ul![
        C!["stages"],
        (1..model.event.stages_count).map(|stage| {
            let current = stage == model.ui.stage;
            view_stage_link(stage, current)
        })
    ]
}

#[allow(dead_code)]
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
