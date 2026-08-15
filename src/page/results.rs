use crate::event::*;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use sycamore::prelude::*;

// Results view.
// Render ResultView
// Model to sorting
// Change Class

pub enum Msg {
    Reload,
    ShowClass(String),
    Publish,
    ToggleCollapse(u8),
    CollapseAll,
    ExpandAll,
    Sort(SortKey),
}

/// What a results-table column sorts by.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortKey {
    /// Car number (#): leading digits compared numerically, remainder as text.
    Car,
    /// Driver name.
    Driver,
    /// A test's stage position.
    TestPos(u8),
    /// A test's cumulative position (O/R).
    TestOr(u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortDir {
    Asc,
    Desc,
}

#[derive(Clone, Copy)]
pub struct Model {
    pub results: Signal<ResultView>,
    /// Test numbers whose run details are currently collapsed.
    pub collapsed: Signal<BTreeSet<u8>>,
    /// Current column sort; defaults to car number ascending.
    pub sort: Signal<(SortKey, SortDir)>,
}

pub fn init(event: &EventInfo, runs: &[RunRecord]) -> Model {
    let results = create_signal(build_view(event, runs, &class_tabs(event)[0]));
    Model {
        results,
        collapsed: create_signal(BTreeSet::new()),
        sort: create_signal((SortKey::Car, SortDir::Asc)),
    }
}

/// Compute results for a tab: Outright = all active entries, others filtered.
fn build_view(event: &EventInfo, runs: &[RunRecord], tab: &str) -> ResultView {
    if tab == "Outright" {
        create_outright_view(event, runs)
    } else {
        create_result_view(event, runs, tab)
    }
}

/// Tab order: Outright always first, then the event's classes (deduped).
fn class_tabs(event: &EventInfo) -> Vec<String> {
    let mut tabs = vec!["Outright".to_string()];
    for c in &event.classes {
        if !tabs.contains(c) {
            tabs.push(c.clone());
        }
    }
    tabs
}

fn load_class(model: crate::Model, class: &str) {
    let event = model.app.event.get_clone();
    let runs = model.app.runs.get_clone();
    let results = build_view(&event, &runs, class);
    model.screens.results.results.set(results);
}

pub fn update(model: crate::Model, msg: Msg) {
    match msg {
        Msg::ShowClass(class) => load_class(model, &class),
        Msg::Reload => {
            let class = model.screens.results.results.with(|r| r.class.clone());
            load_class(model, &class);
        }
        Msg::Publish => {
            #[cfg(target_arch = "wasm32")]
            publish(model);
            #[cfg(not(target_arch = "wasm32"))]
            let _ = model;
        }
        Msg::ToggleCollapse(test) => {
            model.screens.results.collapsed.update(|s| {
                if !s.insert(test) {
                    s.remove(&test);
                }
            });
        }
        Msg::CollapseAll => {
            let n = model
                .screens
                .results
                .results
                .with(|r| r.event.stage_count());
            model.screens.results.collapsed.set((1..=n as u8).collect());
        }
        Msg::ExpandAll => {
            model.screens.results.collapsed.set(BTreeSet::new());
        }
        Msg::Sort(key) => {
            model.screens.results.sort.update(|(k, d)| {
                if *k == key {
                    *d = if *d == SortDir::Asc {
                        SortDir::Desc
                    } else {
                        SortDir::Asc
                    };
                } else {
                    *k = key;
                    *d = SortDir::Asc;
                }
            });
        }
    }
}

/// Broadcast a results snapshot to the timing room (audit trail; each publish
/// is a new message, older versions stay in history).
#[cfg(target_arch = "wasm32")]
fn publish(model: crate::Model) {
    let Some(room) = crate::services::matrix::room() else {
        return;
    };
    let event = model.app.event.get_clone();
    let scores = model.app.scores.get_clone();
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(e) = crate::services::matrix::send_result(&room, &event, &scores).await {
            model.app.conn.set(crate::app::ConnState::Error(e));
        }
    });
}

pub fn view(model: crate::Model) -> View {
    view! {
        (view_publish(model))
        (move || {
            let results = model.screens.results.results.with(|r| r.clone());
            view_results(model, &results)
        })
    }
}

fn view_publish(model: crate::Model) -> View {
    let joined = model.app.room.with(|r| r.is_some());
    view! {
        div(class="box is-hidden-print") {
            div(class="level") {
                div(class="level-left") {
                    div(class="level-item") {
                        h2(class="title is-5") { "Results" }
                    }
                }
                div(class="level-right") {
                    div(class="level-item") {
                        button(
                            class="button is-small",
                            on:click=move |_| crate::update(model, crate::Msg::ResultMsg(Msg::ExpandAll)),
                        ) {
                            span(class="icon") { i(class="fa fa-angle-double-down") }
                            span { "Expand all" }
                        }
                    }
                    div(class="level-item") {
                        button(
                            class="button is-small",
                            on:click=move |_| crate::update(model, crate::Msg::ResultMsg(Msg::CollapseAll)),
                        ) {
                            span(class="icon") { i(class="fa fa-angle-double-up") }
                            span { "Collapse all" }
                        }
                    }
                    div(class="level-item") {
                        button(
                            class="button",
                            on:click=move |_| {
                                if let Some(w) = web_sys::window() {
                                    let _ = w.print();
                                }
                            },
                        ) {
                            span(class="icon") { i(class="fa fa-print") }
                            span { "Print" }
                        }
                    }
                    div(class="level-item") {
                        button(
                            class=format!("button {}", if joined { "is-primary" } else { "is-light" }),
                            disabled=!joined,
                            on:click=move |_| crate::update(model, crate::Msg::ResultMsg(Msg::Publish)),
                        ) {
                            span(class="icon") { i(class="fa fa-paper-plane") }
                            span { "Publish results" }
                        }
                    }
                }
            }
        }
    }
}

fn view_results(model: crate::Model, results: &ResultView) -> View {
    let class_btns = clasess(model, results);
    let header = table_header(model, results);
    let footer = table_footer(model, results);
    let (key, dir) = model.screens.results.sort.with(|s| *s);
    let mut sorted: Vec<&ResultRow> = results.rows.values().collect();
    sorted.sort_by(|a, b| cmp_rows(a, b, key, dir));
    let rows = sorted
        .into_iter()
        .map(|rr| view_row(model, rr))
        .collect::<Vec<View>>();
    let name = results.event.name.clone();
    let class = results.class.clone();
    let date = results.event.event_date.clone();
    let subtitle = if date.is_empty() {
        class
    } else {
        format!("{class} — {date}")
    };

    view! {
        div {
            (class_btns)
            div(class="is-print-only") {
                h1(class="title is-4") { (name) }
                h2(class="subtitle is-5") { (subtitle) }
            }
            div(class="table-container") {
                table(class="table is-bordered is-narrow") {
                    (header)
                    (rows)
                    (footer)
                }
            }
        }
    }
}

/// Compare two rows for the current sort.  Unranked entries (no score in the
/// column) always sort last, whatever the direction.  Ties break by car, then
/// by running order.
fn cmp_rows(a: &ResultRow, b: &ResultRow, key: SortKey, dir: SortDir) -> Ordering {
    let c = match key {
        SortKey::Car => cmp_car(a, b),
        SortKey::Driver => a.entry.name.cmp(&b.entry.name),
        SortKey::TestPos(test) => cmp_pos(opt_pos(a, test), opt_pos(b, test), dir),
        SortKey::TestOr(test) => cmp_pos(opt_cum(a, test), opt_cum(b, test), dir),
    };
    let c = match key {
        SortKey::Car | SortKey::Driver => {
            if dir == SortDir::Desc {
                c.reverse()
            } else {
                c
            }
        }
        _ => c,
    };
    c.then_with(|| cmp_car(a, b))
        .then_with(|| entry_sort_key(&a.entry).cmp(&entry_sort_key(&b.entry)))
}

/// Car numbers compare by their leading digit run (so 2 < 10), then the rest.
fn cmp_car(a: &ResultRow, b: &ResultRow) -> Ordering {
    num_car_key(&a.entry.car).cmp(&num_car_key(&b.entry.car))
}

fn num_car_key(car: &str) -> (u32, &str) {
    let digits = car.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        (0, car)
    } else {
        (car[..digits].parse::<u32>().unwrap_or(0), &car[digits..])
    }
}

/// Ranked values compare in the given direction; unranked (None) sorts last.
fn cmp_pos(a: Option<u8>, b: Option<u8>, dir: SortDir) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(x), Some(y)) => {
            let c = x.cmp(&y);
            if dir == SortDir::Desc {
                c.reverse()
            } else {
                c
            }
        }
    }
}

fn opt_pos(row: &ResultRow, test: u8) -> Option<u8> {
    row.columns
        .get(test as usize - 1)
        .and_then(|rs| rs.as_ref())
        .and_then(|rs| rs.stage_pos.as_ref())
        .map(|p| p.pos)
}

fn opt_cum(row: &ResultRow, test: u8) -> Option<u8> {
    row.columns
        .get(test as usize - 1)
        .and_then(|rs| rs.as_ref())
        .and_then(|rs| rs.cum_pos.as_ref())
        .map(|p| p.pos)
}

/// The final test of the event.
fn last_test(model: crate::Model) -> u8 {
    model
        .screens
        .results
        .results
        .with(|r| r.event.stage_count() as u8)
}

/// Column count for a collapsed test's per-test block.  The last test keeps
/// its O/R column (final ordering for officials), so it spans 3.
fn collapsed_span(model: crate::Model, test: u8) -> usize {
    if test == last_test(model) {
        3
    } else {
        2
    }
}

/// Footer under each test: the stage base time and the derived no-time scores
/// (WD / FTS / DNF = base + 5s, DNS = base + 10s) so officials can see what
/// aborted runs are worth.
fn table_footer(model: crate::Model, results: &ResultView) -> View {
    let mut cells: Vec<View> = vec![view! { td(colspan="2") { "No-time" } }];
    for i in 0..results.event.stage_count() {
        let test = i as u8 + 1;
        if model.screens.results.collapsed.with(|c| c.contains(&test)) {
            cells.push(view! { td(colspan=collapsed_span(model, test).to_string()) {} });
            continue;
        }
        let base = results.base_times_ds.get(i).copied().unwrap_or(0);
        let lines: Vec<View> = if base == 0 {
            vec![view! { div(class="has-text-grey-light") { "\u{2014}" } }]
        } else {
            let base_s = base as f32 / 10.0;
            let wd_s = (base as u32 + 50) as f32 / 10.0;
            let dns_s = (base as u32 + 100) as f32 / 10.0;
            vec![
                view! { div { ("Base ") (format!("{base_s:.1}")) } },
                view! { div { ("WD / FTS / DNF ") (format!("{wd_s:.1}")) } },
                view! { div { ("DNS ") (format!("{dns_s:.1}")) } },
            ]
        };
        let cell = view! { td(colspan="5") { (lines) } };
        cells.push(cell);
    }
    view! { tfoot { tr(class="is-together-print") { (cells) } } }
}

const COLS_PER_TEST: usize = 5;

fn view_row(model: crate::Model, rr: &ResultRow) -> View {
    let car = rr.entry.car.clone();
    let name = rr.entry.name.clone();
    let columns = rr
        .columns
        .iter()
        .enumerate()
        .map(|(i, rso)| show_rs(model, i as u8 + 1, rso))
        .collect::<Vec<View>>();
    view! {
        tr(class="is-together-print") {
            td { (car) }
            td { (name) }
            (columns)
        }
    }
}

fn show_rs(model: crate::Model, test: u8, rso: &Option<ResultScore>) -> View {
    let collapsed = model.screens.results.collapsed.with(|c| c.contains(&test));
    match rso {
        Some(rs) => {
            let runs = show_runs(rs);
            let pos = match &rs.stage_pos {
                Some(p) => {
                    let pos = p.pos.to_string();
                    view! { td { (pos) } }
                }
                None => view! { td {} },
            };
            if collapsed {
                // Collapsed: keep the full Time cell (all runs, struck ones
                // included) and Pos; drop Score and Cum.  The last test keeps
                // its O/R column as well.
                let or = if test == last_test(model) {
                    let cell = cum_or(&rs.cum_pos);
                    view! { td { (cell) } }
                } else {
                    view! {}
                };
                view! {
                    td { (runs) }
                    (pos)
                    (or)
                }
            } else {
                let (score, cum) = match &rs.stage_pos {
                    Some(p) => {
                        let s = format!("{:.1}", p.score_ds as f32 / 10.0);
                        let cum = match &rs.cum_pos {
                            Some(cp) => {
                                let cs = format!("{:.1}", cp.score_ds as f32 / 10.0);
                                view! { td { (cs) } }
                            }
                            None => view! { td(class="has-text-grey-light") { "\u{2014}" } },
                        };
                        (view! { td { (s) } }, cum)
                    }
                    None => (
                        view! { td(class="has-text-grey-light") { "\u{2014}" } },
                        view! { td(class="has-text-grey-light") { "\u{2014}" } },
                    ),
                };
                let cum_or_cell = cum_or(&rs.cum_pos);
                view! {
                    td { (runs) }
                    (score)
                    (pos)
                    (cum)
                    td { (cum_or_cell) }
                }
            }
        }
        None => {
            let n = if collapsed {
                collapsed_span(model, test)
            } else {
                COLS_PER_TEST
            };
            let tds = (0..n).map(|_| view! { td {} }).collect::<Vec<View>>();
            view! { (tds) }
        }
    }
}

/// The car's runs in this test: all times comma-separated in run order, with
/// the non-counting runs struck out (dropped by best-X, or on a cancelled
/// stage) and the fastest counting run highlighted in green.
fn show_runs(rs: &ResultScore) -> View {
    let any_counted = rs.runs.iter().any(|r| r.counted);
    let fastest = if any_counted {
        rs.runs.iter().filter(|r| r.counted).map(|r| r.score).min()
    } else {
        None
    };
    let mut cells: Vec<View> = Vec::new();
    for (i, r) in rs.runs.iter().enumerate() {
        if i > 0 {
            cells.push(view! { span { ", " } });
        }
        let content = run_time_text(&r.time);
        let cell = if !r.counted {
            view! { span(class="kt-struck has-text-grey-light") { (content) } }
        } else if Some(r.score) == fastest {
            view! { span(class="has-text-success has-text-weight-bold") { (content) } }
        } else {
            view! { span { (content) } }
        };
        cells.push(cell);
    }
    view! { div { (cells) } }
}

/// Compact per-run time text: `12.3` with small flag/garage glyphs, or the
/// status code (DNS / DNF / FTS / WD) for aborted runs.
fn run_time_text(time: &crate::event::KTime) -> View {
    match time {
        crate::event::KTime::Time(t) => {
            let ts = format!("{:.1}", t.time_ds as f32 / 10.0);
            let mut icons: Vec<View> = vec![];
            if t.garage {
                icons.push(view! { i(class="fa fa-warehouse") });
            }
            for _ in 0..t.flags {
                icons.push(view! { i(class="fa fa-flag") });
            }
            view! { span { (ts) (icons) } }
        }
        crate::event::KTime::NOSHO => view! { span { "DNS" } },
        crate::event::KTime::WD => view! { span { "WD" } },
        crate::event::KTime::FTS => view! { span { "FTS" } },
        crate::event::KTime::DNF => view! { span { "DNF" } },
    }
}

fn cum_or(cum: &Option<Pos>) -> View {
    match cum {
        None => view! { div(class="has-text-grey-light") { "\u{2014}" } },
        Some(p) => {
            let pos = p.pos.to_string();
            let delta = if p.change == 0 {
                view! {}
            } else {
                let class = if p.change > 0 {
                    "kt-or-delta has-text-success"
                } else {
                    "kt-or-delta has-text-danger"
                };
                let s = format!("{:+}", p.change);
                view! { span(class=class) { (s) } }
            };
            view! { div { (pos) (delta) } }
        }
    }
}

fn clasess(model: crate::Model, results: &ResultView) -> View {
    let current = results.class.clone();
    let tabs = class_tabs(&results.event);
    let btns = tabs
        .iter()
        .map(|class| {
            let class = class.clone();
            let class_disp = class.clone();
            let active = class == current;
            view! {
                button(
                    class=format!(
                        "button is-hidden-print {}",
                        if active { "is-link is-selected" } else { "is-light" }
                    ),
                    on:click=move |_| crate::update(model, crate::Msg::ResultMsg(Msg::ShowClass(class.clone()))),
                ) {
                    (class_disp)
                }
            }
        })
        .collect::<Vec<View>>();
    view! { (btns) }
}

fn table_header(model: crate::Model, results: &ResultView) -> View {
    let stages_count = results.event.stage_count();
    // Precompute the per-test labels (stage names, falling back to "Test N").
    let labels: Vec<String> = (0..stages_count)
        .map(|i| {
            let name = results.event.stage(i).name;
            if name.is_empty() {
                format!("Test {}", i + 1)
            } else {
                name
            }
        })
        .collect();
    let mut first_row: Vec<View> = vec![view! { th(colspan="2") { "Entry" } }];
    for (i, label) in labels.into_iter().enumerate() {
        let test = i as u8 + 1;
        let collapsed = model.screens.results.collapsed.with(|c| c.contains(&test));
        let chevron = if collapsed {
            "fa-chevron-right"
        } else {
            "fa-chevron-down"
        };
        let span = if collapsed {
            collapsed_span(model, test).to_string()
        } else {
            "5".to_string()
        };
        first_row.push(view! {
            th(
                colspan=span,
                class="kt-test-header",
                on:click=move |_| crate::update(model, crate::Msg::ResultMsg(Msg::ToggleCollapse(test))),
            ) {
                (label)
                span(class="icon is-small") { i(class=format!("fa {chevron}")) }
            }
        });
    }
    let mut head: Vec<View> = vec![];
    head.push(view! {
        tr {
            (first_row)
        }
    });
    // per test: collapsed -> Time Pos; expanded -> Time Score Pos Cum O/R
    head.push(view! {
        tr {
            th(
                class="kt-sortable",
                on:click=move |_| crate::update(model, crate::Msg::ResultMsg(Msg::Sort(SortKey::Car))),
            ) {
                "#"
                (sort_icon(model, SortKey::Car))
            }
            th(
                class="kt-sortable",
                on:click=move |_| crate::update(model, crate::Msg::ResultMsg(Msg::Sort(SortKey::Driver))),
            ) {
                "Driver"
                (sort_icon(model, SortKey::Driver))
            }
            ((0..stages_count)
                .map(|i| {
                    let test = i as u8 + 1;
                    if model.screens.results.collapsed.with(|c| c.contains(&test)) {
                        view! {
                            th { "Time" }
                            th(
                                class="kt-sortable",
                                on:click=move |_| crate::update(model, crate::Msg::ResultMsg(Msg::Sort(SortKey::TestPos(test)))),
                            ) {
                                "Pos"
                                (sort_icon(model, SortKey::TestPos(test)))
                            }
                            (if test == last_test(model) {
                                view! {
                                    th(
                                        class="kt-sortable",
                                        on:click=move |_| crate::update(model, crate::Msg::ResultMsg(Msg::Sort(SortKey::TestOr(test)))),
                                    ) {
                                        "O/R"
                                        (sort_icon(model, SortKey::TestOr(test)))
                                    }
                                }
                            } else {
                                view! {}
                            })
                        }
                    } else {
                        view! {
                            th { "Time" }
                            th { "Score" }
                            th(
                                class="kt-sortable",
                                on:click=move |_| crate::update(model, crate::Msg::ResultMsg(Msg::Sort(SortKey::TestPos(test)))),
                            ) {
                                "Pos"
                                (sort_icon(model, SortKey::TestPos(test)))
                            }
                            th { "Cum" }
                            th(
                                class="kt-sortable",
                                on:click=move |_| crate::update(model, crate::Msg::ResultMsg(Msg::Sort(SortKey::TestOr(test)))),
                            ) {
                                "O/R"
                                (sort_icon(model, SortKey::TestOr(test)))
                            }
                        }
                    }
                })
                .collect::<Vec<View>>())
        }
    });
    view! { (head) }
}

/// Up/down arrow on the column the table is currently sorted by.
fn sort_icon(model: crate::Model, key: SortKey) -> View {
    let (k, d) = model.screens.results.sort.with(|s| *s);
    if k == key {
        let icon = if d == SortDir::Asc {
            "fa-sort-up"
        } else {
            "fa-sort-down"
        };
        view! { span(class="icon is-small") { i(class=format!("fa {icon}")) } }
    } else {
        view! {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(car: &str, name: &str) -> ResultRow {
        let mut e = Entry::new(car, name);
        e.entry_no = 0;
        ResultRow {
            entry: e,
            columns: vec![],
        }
    }

    fn pos(p: u8) -> Option<ResultScore> {
        Some(ResultScore {
            stage_pos: Some(Pos {
                pos: p,
                ..Default::default()
            }),
            ..Default::default()
        })
    }

    fn with_pos(mut r: ResultRow, p: u8) -> ResultRow {
        r.columns = vec![pos(p)];
        r
    }

    fn cars(rows: &[ResultRow], key: SortKey, dir: SortDir) -> Vec<String> {
        let mut v: Vec<&ResultRow> = rows.iter().collect();
        v.sort_by(|a, b| cmp_rows(a, b, key, dir));
        v.iter().map(|r| r.entry.car.clone()).collect()
    }

    #[test]
    fn car_sort_is_numeric() {
        let rows = vec![row("10", "a"), row("2", "b"), row("1", "c")];
        assert_eq!(cars(&rows, SortKey::Car, SortDir::Asc), ["1", "2", "10"]);
        assert_eq!(cars(&rows, SortKey::Car, SortDir::Desc), ["10", "2", "1"]);
    }

    #[test]
    fn driver_sort_is_by_name() {
        let rows = vec![row("1", "zed"), row("2", "adam")];
        assert_eq!(cars(&rows, SortKey::Driver, SortDir::Asc), ["2", "1"]);
        assert_eq!(cars(&rows, SortKey::Driver, SortDir::Desc), ["1", "2"]);
    }

    #[test]
    fn unranked_sort_last_both_directions() {
        let rows = vec![
            row("5", "no-score"),
            with_pos(row("1", "first"), 1),
            with_pos(row("3", "second"), 2),
        ];
        assert_eq!(
            cars(&rows, SortKey::TestPos(1), SortDir::Asc),
            ["1", "3", "5"]
        );
        assert_eq!(
            cars(&rows, SortKey::TestPos(1), SortDir::Desc),
            ["3", "1", "5"]
        );
    }

    #[test]
    fn pos_tie_broken_by_car() {
        let rows = vec![with_pos(row("5", "a"), 1), with_pos(row("3", "b"), 1)];
        assert_eq!(cars(&rows, SortKey::TestPos(1), SortDir::Asc), ["3", "5"]);
    }
}
