use enclose::enc;
use indexmap::{IndexMap, IndexSet};
use seed::{prelude::*, *};
use serde::{Deserialize, Serialize};
use std::{
    // async_iter::from_iter,
    collections::{HashMap, HashSet},
    mem,
};
use uuid::Uuid;
use web_sys::HtmlInputElement;

const ENTER_KEY: u32 = 13;
const ESC_KEY: u32 = 27;
const STORAGE_KEY: &str = "seed-todomvc";
const EVENT_STORAGE_KEY: &str = "kts-event";

type TodoId = Uuid;

// ------ ------
//     Init
// ------ ------

fn init(url: Url, orders: &mut impl Orders<Msg>) -> Model {
    orders
        .subscribe(Msg::UrlChanged)
        .notify(subs::UrlChanged(url));

    Model {
        data: LocalStorage::get(STORAGE_KEY).unwrap_or_default(),
        refs: Refs::default(),
        event: LocalStorage::get(EVENT_STORAGE_KEY).unwrap_or_default(), //Event::default(), // { name: (), times: (), scores: (), classes: (), entries: (), filter: (), new_todo_title: (), editing_todo: () }
    }
}

// ------ ------
//     Models
// ------ ------

// ------ Model ------

struct Model {
    data: Data,
    refs: Refs,
    event: Event,
}

#[derive(Serialize, Deserialize)]
struct Event {
    name: String,

    times: Vec<RawScore>, // raw times, order of insertion
    scores: HashMap<i8, HashMap<String, CalcScore>>, // calculated for display.  Key is [stage][car] holding a Score.
    classes: IndexSet<String>,                       // list of known classes. Order as per display
    entries: IndexMap<String, Entry>, // list of know entrants/drivers. Ordered by car number

                                      // IndexMap<TodoId, Todo>,
                                      // filter: TodoFilter,
                                      // new_todo_title: String,
                                      // editing_todo: Option<EditingTodo>,
}

impl Default for Event {
    fn default() -> Self {
        // let letters: IndexSet<_> = "a short treatise on fungi".chars().collect();
        let c = ["Outright", "Female", "Junior"];
        let args = c.iter().map(|&s| s.into()).collect();

        // let a: Vec<_> = vec!["Outright", "Female", "Junior"];
        // a.t .map(|a|) {a. to_string};
        // let b: IndexSet<_> = a.iter().collect(); // ndexSet::from_iter(a);
        // let d = b;
        let classes: IndexSet<String> = args;
        Self {
            name: "Event TBA2".to_owned(),
            times: Default::default(),
            scores: Default::default(),
            // classes: IndexSet::from_iter(vec!["Outright", "Female", "Junior" ]),
            entries: Default::default(),
            classes,
            // classes: todo!(),
            // entries: todo!(),
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Data {
    todos: IndexMap<TodoId, Todo>,
    filter: TodoFilter,
    new_todo_title: String,
    editing_todo: Option<EditingTodo>,
}

#[derive(Default)]
struct Refs {
    editing_todo_input: ElRef<HtmlInputElement>,
}

// ------ Todo ------
// WD, wronmg direction
// DNS, dis not start
// FTS failed to stop
// DNF did not finish
#[derive(Default, Serialize, Deserialize)]
enum Time {
    #[default]
    DNS,
    WD,
    FTS,
    DNF,
    Time(f32), // seconds
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
#[derive(Default, Serialize, Deserialize)]
struct RawScore {
    /// data entry.  P
    stage: i8,
    car: String,
    time: Time, // as entered.. maybe an enum? of codes and time?
    flags: i8,
    ignore: bool, // set when should be used in results.  Ignored if not current, i.e. has been replaced
    when: String, // timestamp of create/edit
    by: String,   // user id
}
#[derive(Default, Serialize, Deserialize, PartialEq, Eq)]
struct Pos {
    order: i8,                    // overall pos, for sorting.. might not be required?
    pos: HashMap<String, String>, // columname/Classname vs position. Posn is String for =2nd and suchlike
}

#[derive(Default, Serialize, Deserialize)]
struct CalcScore {
    // data entry.  P
    stage: Vec<Pos>,
    outright: Vec<Pos>,
    // pos: i8,       // outright position
    // stage_pos: i8, // position in stage
    // number i8,
    car: String,
    time: Time, // as entered.. maybe an enum? of codes and time? pritable, so time plus penalties etc.
    flags: i8,
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

// impl Entry {}

// ------ Todo ------

#[derive(Serialize, Deserialize, PartialEq, Eq)]
struct Todo {
    title: String,
    completed: bool,
}

// ------ EditingTodo ------

#[derive(Serialize, Deserialize)]
struct EditingTodo {
    id: TodoId,
    title: String,
}

// ------ TodoFilter ------

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum TodoFilter {
    All,
    Active,
    Completed,
}

impl Default for TodoFilter {
    fn default() -> Self {
        Self::All
    }
}

impl TodoFilter {
    const fn to_url_path(self) -> &'static str {
        match self {
            Self::All => "",
            Self::Active => "active",
            Self::Completed => "completed",
        }
    }
}

// ------ ------
//    Update
// ------ ------

enum Msg {
    UrlChanged(subs::UrlChanged),

    NewTodoTitleChanged(String),
    ClearCompleted,
    ToggleAll,

    CreateNewTodo,
    ToggleTodo(TodoId),
    RemoveTodo(TodoId),

    StartTodoEdit(TodoId),
    EditingTodoTitleChanged(String),
    SaveEditingTodo,
    CancelTodoEdit,
}

fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    let data = &mut model.data;
    match msg {
        Msg::UrlChanged(subs::UrlChanged(mut url)) => {
            data.filter = match url.next_path_part() {
                Some(path_part) if path_part == TodoFilter::Active.to_url_path() => {
                    TodoFilter::Active
                }
                Some(path_part) if path_part == TodoFilter::Completed.to_url_path() => {
                    TodoFilter::Completed
                }
                _ => TodoFilter::All,
            };
        }
        Msg::NewTodoTitleChanged(title) => {
            data.new_todo_title = title;
        }
        Msg::ClearCompleted => {
            data.todos.retain(|_, todo| !todo.completed);
        }
        Msg::ToggleAll => {
            let all_todos_completed = data.todos.values().all(|todo| todo.completed);

            for (_, todo) in &mut data.todos {
                todo.completed = !all_todos_completed
            }
        }

        Msg::CreateNewTodo => {
            data.todos.insert(
                TodoId::new_v4(),
                Todo {
                    title: mem::take(&mut data.new_todo_title),
                    completed: false,
                },
            );
        }
        Msg::ToggleTodo(todo_id) => {
            if let Some(todo) = data.todos.get_mut(&todo_id) {
                todo.completed = !todo.completed;
            }
        }
        Msg::RemoveTodo(todo_id) => {
            data.todos.shift_remove(&todo_id);
        }

        Msg::StartTodoEdit(todo_id) => {
            if let Some(todo) = data.todos.get(&todo_id) {
                data.editing_todo = Some({
                    EditingTodo {
                        id: todo_id,
                        title: todo.title.clone(),
                    }
                });
            }

            let input = model.refs.editing_todo_input.clone();
            orders.after_next_render(move |_| {
                input.get().expect("get `editing_todo_input`").select();
            });
        }
        Msg::EditingTodoTitleChanged(title) => {
            if let Some(ref mut editing_todo) = data.editing_todo {
                editing_todo.title = title
            }
        }
        Msg::SaveEditingTodo => {
            if let Some(editing_todo) = data.editing_todo.take() {
                if let Some(todo) = data.todos.get_mut(&editing_todo.id) {
                    todo.title = editing_todo.title;
                }
            }
        }
        Msg::CancelTodoEdit => {
            data.editing_todo = None;
        }
    }
    // Note: It should be optimized in a real-world application.
    LocalStorage::insert(STORAGE_KEY, &data).expect("save data to LocalStorage");
}

// ------ ------
//     View
// ------ ------

fn view(model: &Model) -> impl IntoNodes<Msg> {
    let data = &model.data;
    nodes![
        view_header(&data.new_todo_title),
        if data.todos.is_empty() {
            vec![]
        } else {
            vec![
                view_main(
                    &data.todos,
                    data.filter,
                    &data.editing_todo,
                    &model.refs.editing_todo_input,
                ),
                view_footer(&data.todos, data.filter),
                view_event(&model.event),
            ]
        },
    ]
}

// ------ header ------

fn view_header(new_todo_title: &str) -> Node<Msg> {
    header![
        C!["header"],
        h1!["todos"],
        input![
            C!["new-todo"],
            attrs! {
                At::Placeholder => "What needs to be done?";
                At::AutoFocus => true.as_at_value();
                At::Value => new_todo_title;
            },
            keyboard_ev(Ev::KeyDown, |keyboard_event| {
                IF!(keyboard_event.key_code() == ENTER_KEY => Msg::CreateNewTodo)
            }),
            input_ev(Ev::Input, Msg::NewTodoTitleChanged),
        ]
    ]
}

// ------ main ------

fn view_main(
    todos: &IndexMap<TodoId, Todo>,
    filter: TodoFilter,
    editing_todo: &Option<EditingTodo>,
    editing_todo_input: &ElRef<HtmlInputElement>,
) -> Node<Msg> {
    let all_todos_completed = todos.values().all(|todo| todo.completed);

    section![
        C!["main"],
        input![
            id!("toggle-all"),
            C!["toggle-all"],
            attrs! {
                At::Type => "checkbox",
                At::Checked => all_todos_completed.as_at_value(),
            },
            ev(Ev::Click, |_| Msg::ToggleAll)
        ],
        label![attrs! {At::For => "toggle-all"}, "Mark all as complete"],
        view_todos(todos, filter, editing_todo, editing_todo_input)
    ]
}

fn view_event(event: &Event) -> Node<Msg> {
    ul![div!("hello"), div!(event.name.clone())]
}

fn view_todos(
    todos: &IndexMap<TodoId, Todo>,
    filter: TodoFilter,
    editing_todo: &Option<EditingTodo>,
    editing_todo_input: &ElRef<HtmlInputElement>,
) -> Node<Msg> {
    ul![
        C!["todo-list"],
        todos.iter().filter_map(|(todo_id, todo)| {
            let show_todo = match filter {
                TodoFilter::All => true,
                TodoFilter::Active => !todo.completed,
                TodoFilter::Completed => todo.completed,
            };
            IF!(show_todo => view_todo(todo_id, todo, editing_todo, editing_todo_input))
        })
    ]
}

#[allow(clippy::cognitive_complexity)]
fn view_todo(
    todo_id: &TodoId,
    todo: &Todo,
    editing_todo: &Option<EditingTodo>,
    editing_todo_input: &ElRef<HtmlInputElement>,
) -> Node<Msg> {
    li![
        C![
            IF!(todo.completed => "completed"),
            IF!(matches!(editing_todo, Some(editing_todo) if &editing_todo.id == todo_id) => "editing"),
        ],
        div![
            C!["view"],
            input![
                C!["toggle"],
                attrs! {
                    At::Type => "checkbox",
                    At::Checked => todo.completed.as_at_value()
                },
                ev(
                    Ev::Change,
                    enc!((todo_id) move |_| Msg::ToggleTodo(todo_id))
                )
            ],
            label![
                ev(
                    Ev::DblClick,
                    enc!((todo_id) move |_| Msg::StartTodoEdit(todo_id))
                ),
                &todo.title
            ],
            button![
                C!["destroy"],
                ev(Ev::Click, enc!((todo_id) move |_| Msg::RemoveTodo(todo_id)))
            ]
        ],
        match editing_todo {
            Some(editing_todo) if &editing_todo.id == todo_id => {
                input![
                    el_ref(editing_todo_input),
                    C!["edit"],
                    attrs! {At::Value => editing_todo.title},
                    ev(Ev::Blur, |_| Msg::SaveEditingTodo),
                    input_ev(Ev::Input, Msg::EditingTodoTitleChanged),
                    keyboard_ev(Ev::KeyDown, |keyboard_event| {
                        match keyboard_event.key_code() {
                            ENTER_KEY => Some(Msg::SaveEditingTodo),
                            ESC_KEY => Some(Msg::CancelTodoEdit),
                            _ => None,
                        }
                    }),
                ]
            }
            _ => empty![],
        }
    ]
}

// ------ footer ------

fn view_footer(todos: &IndexMap<TodoId, Todo>, filter: TodoFilter) -> Node<Msg> {
    let active_count = todos.values().filter(|todo| !todo.completed).count();

    footer![
        C!["footer"],
        span![
            C!["todo-count"],
            strong![active_count.to_string()],
            span![format!(
                " item{} left",
                if active_count == 1 { "" } else { "s" }
            )]
        ],
        view_filters(filter),
        view_clear_completed(todos),
    ]
}

fn view_filters(filter: TodoFilter) -> Node<Msg> {
    ul![
        C!["filters"],
        view_filter("All", TodoFilter::All, filter),
        view_filter("Active", TodoFilter::Active, filter),
        view_filter("Completed", TodoFilter::Completed, filter),
    ]
}

fn view_filter(title: &str, filter: TodoFilter, current_filter: TodoFilter) -> Node<Msg> {
    li![a![
        C![IF!(filter == current_filter => "selected")],
        attrs! {
            At::Href => format!("/{}", filter.to_url_path())
        },
        style! {St::Cursor => "pointer"},
        title
    ]]
}

fn view_clear_completed(todos: &IndexMap<TodoId, Todo>) -> Option<Node<Msg>> {
    let completed_count = todos.values().filter(|todo| todo.completed).count();

    IF!(completed_count > 0 => {
        button![
            C!["clear-completed"],
            ev(Ev::Click, |_| Msg::ClearCompleted),
            format!("Clear completed ({})", completed_count),
        ]
    })
}

// WD, wronmg direction
// DNS, dis not start
// FTS failed to stop
// DNF did not finish

// The application of penalties must be as follows for each infringement:
// Penalty Condition Penalty Applied
// (i) Wrong direction Slowest time plus five (5) seconds
// (ii) Any other action that can be deemed as incorrectly
// completing that course (such as reversing after exceed the
// limits of a garage)
// Slowest time plus five (5) seconds

// (iii) Failure to complete a test Slowest time plus five (5) seconds
// (iv) Running out of order (without the prior approval of the Clerk
// of the Course)
// Slowest time plus five (5) seconds

// (v) Failing to stop completely within a mid-course garage Slowest time plus five (5) seconds
// (vi) Failing to stop completely at the finish of a test Slowest time plus five (5) seconds
// (vii) Finish a test with the car stopped but completely
// outside the garage
// Slowest time plus five (5) seconds

// (viii) Finishing a test with part of the car outside the
// garage boundaries (plus the penalty for striking any
// flag/marker
// Plus five (5) seconds plus any

// flag/marker strike
// (ix) Striking a course flag/marker(including garage boundary
// flag/marker)
// Plus five (5) seconds per flag/marker
// (x) Failure to attempt a test Slowest time plus ten (10) seconds

// ------ ------
//     Start
// ------ ------

fn main() {
    App::start("app", init, update, view);
}
