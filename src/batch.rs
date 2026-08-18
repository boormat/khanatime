//! Staged-edit batching for officials.
//!
//! While in edit/admin mode, edits are queued locally (staged) instead of
//! being sent.  On Save the batch is compacted to remove redundant edits,
//! diffed against the committed state for confirmation, then emitted as entry
//! messages (or one setup manifest, for the Event Admin page).  Pure + testable.

use crate::event::{Entry, EventInfo, Stage, TimingStyle};

/// A staged entry edit (admin edit mode on the Entries page).
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
#[allow(dead_code)] // used from entry_app + tests only
pub enum EditOp {
    /// Insert or replace the entry (keyed by car number).
    Upsert(Entry),
    /// Tombstone: remove the entry by car number.
    Delete(String),
}

#[allow(dead_code)] // used from entry_app + tests only
fn op_key(op: &EditOp) -> String {
    match op {
        EditOp::Upsert(e) => e.car.clone(),
        EditOp::Delete(car) => car.clone(),
    }
}

#[allow(dead_code)] // used from entry_app + tests only
fn upsert(entries: &mut Vec<Entry>, entry: Entry) {
    if let Some(existing) = entries.iter_mut().find(|e| e.car == entry.car) {
        *existing = entry;
    } else {
        entries.push(entry);
    }
}

/// Fold staged ops over the committed entry list (WYSIWYG preview while
/// editing).
#[allow(dead_code)] // used from entry_app + tests only
pub fn apply_ops(entries: &[Entry], ops: &[EditOp]) -> Vec<Entry> {
    let mut out: Vec<Entry> = entries.to_vec();
    for op in ops {
        match op {
            EditOp::Upsert(e) => upsert(&mut out, e.clone()),
            EditOp::Delete(car) => out.retain(|e| e.car != *car),
        }
    }
    out
}

/// Collapse a staged batch into the minimal set of messages to send:
/// keep only the last op per entry (each upsert is a full snapshot, so the
/// last one fully determines that entry's final state), then drop no-ops
/// against the committed state (an upsert already present, or a tombstone for
/// an entry that isn't committed).  Order is stable, first-seen.
#[allow(dead_code)] // used from entry_app + tests only
pub fn compact_ops(ops: &[EditOp], current: &[Entry]) -> Vec<EditOp> {
    let mut kept: Vec<EditOp> = Vec::new();
    for op in ops {
        kept.retain(|o| op_key(o) != op_key(op));
        kept.push(op.clone());
    }
    kept.retain(|op| match op {
        EditOp::Upsert(e) => !current.iter().any(|c| c.car == e.car && c == e),
        EditOp::Delete(car) => current.iter().any(|c| c.car == *car),
    });
    kept
}

fn class_delta(before: &[String], after: &[String]) -> (Vec<String>, Vec<String>) {
    let added: Vec<String> = after
        .iter()
        .filter(|c| !before.contains(c))
        .cloned()
        .collect();
    let removed: Vec<String> = before
        .iter()
        .filter(|c| !after.contains(c))
        .cloned()
        .collect();
    (added, removed)
}

#[allow(dead_code)] // used from entry_app + tests only
fn car_or_unassigned(car: &str) -> String {
    if car.is_empty() {
        "(unassigned)".to_string()
    } else {
        car.to_string()
    }
}

fn shared_str(s: &Option<String>) -> String {
    match s.as_deref() {
        Some(x) if !x.trim().is_empty() => x.trim().to_string(),
        _ => "(none)".to_string(),
    }
}

fn opt_str(s: &Option<String>) -> String {
    match s.as_deref() {
        Some(x) if !x.trim().is_empty() => x.trim().to_string(),
        _ => "(none)".to_string(),
    }
}

fn desc_str(s: &Option<String>) -> String {
    match s.as_deref() {
        Some(x) if !x.trim().is_empty() => x.trim().to_string(),
        _ => "(none)".to_string(),
    }
}

/// Human-readable list of changes a batch of entry ops makes, for the confirm
/// dialog.  Runs over the already-compacted ops.
#[allow(dead_code)] // used from entry_app + tests only
pub fn entry_diff(ops: &[EditOp], current: &[Entry]) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for op in ops {
        match op {
            EditOp::Upsert(e) => match current.iter().find(|c| c.car == e.car) {
                None => lines.push(format!(
                    "+ {} — new entry (number {})",
                    e.name,
                    car_or_unassigned(&e.car)
                )),
                Some(cur) => {
                    if cur.name != e.name {
                        lines.push(format!(
                            "~ {} — name: {} \u{2192} {}",
                            e.car, cur.name, e.name
                        ));
                    }
                    if cur.shared != e.shared {
                        lines.push(format!(
                            "~ {} — shared car: {} \u{2192} {}",
                            e.car,
                            shared_str(&cur.shared),
                            shared_str(&e.shared)
                        ));
                    }
                    if cur.description != e.description {
                        lines.push(format!(
                            "~ {} — description: {} \u{2192} {}",
                            e.car,
                            desc_str(&cur.description),
                            desc_str(&e.description)
                        ));
                    }
                    if cur.vehicle != e.vehicle {
                        lines.push(format!(
                            "~ {} — vehicle: {} \u{2192} {}",
                            e.car,
                            opt_str(&cur.vehicle),
                            opt_str(&e.vehicle)
                        ));
                    }
                    let (added, removed) = class_delta(&cur.classes, &e.classes);
                    if !removed.is_empty() {
                        lines.push(format!(
                            "~ {} — classes: \u{2212}{}",
                            e.car,
                            removed.join(", ")
                        ));
                    }
                    if !added.is_empty() {
                        lines.push(format!("~ {} — classes: +{}", e.car, added.join(", ")));
                    }
                }
            },
            EditOp::Delete(car) => {
                if let Some(e) = current.iter().find(|c| c.car == *car) {
                    lines.push(format!(
                        "\u{2212} Car {} {} (removed)",
                        car_or_unassigned(&e.car),
                        e.name
                    ));
                } else {
                    lines.push(format!("\u{2212} (unknown car {car})"));
                }
            }
        }
    }
    lines
}

fn show(s: &str) -> String {
    if s.is_empty() {
        "(empty)".to_string()
    } else {
        s.to_string()
    }
}

fn stage_desc(s: &Stage) -> String {
    let style = match s.timing {
        TimingStyle::Stopwatch => "Stopwatch",
        TimingStyle::Rally => "Rally",
    };
    format!(
        "Test {}: {} — {} of {} scored, {}",
        s.num, s.name, s.runs_scored, s.runs_total, style
    )
}

/// Human-readable list of changes from one event config to another, for the
/// Event Admin confirm dialog.
fn field_diff(lines: &mut Vec<String>, label: &str, a: &str, b: &str) {
    if a != b {
        lines.push(format!("~ {label}: {} → {}", show(a), show(b)));
    }
}

pub fn event_diff(base: &EventInfo, staged: &EventInfo) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    field_diff(
        &mut lines,
        "Club / district",
        &base.sponsoring_club,
        &staged.sponsoring_club,
    );
    field_diff(&mut lines, "Name", &base.name, &staged.name);
    field_diff(&mut lines, "Year", &base.year, &staged.year);
    field_diff(
        &mut lines,
        "Event date",
        &base.event_date,
        &staged.event_date,
    );
    field_diff(
        &mut lines,
        "Parent rooms",
        &base.parent_rooms.join(", "),
        &staged.parent_rooms.join(", "),
    );
    field_diff(
        &mut lines,
        "Homeservers",
        &base.event_homeservers.join(", "),
        &staged.event_homeservers.join(", "),
    );
    field_diff(
        &mut lines,
        "Event admins",
        &base.event_admins.join(", "),
        &staged.event_admins.join(", "),
    );
    field_diff(
        &mut lines,
        "Owner",
        base.owner.as_deref().unwrap_or(""),
        staged.owner.as_deref().unwrap_or(""),
    );

    let (added, removed) = class_delta(&base.classes, &staged.classes);
    if !removed.is_empty() {
        lines.push(format!("− Classes: {}", removed.join(", ")));
    }
    if !added.is_empty() {
        lines.push(format!("+ Classes: {}", added.join(", ")));
    }

    for s in &base.stages {
        if !staged.stages.iter().any(|x| x.num == s.num) {
            lines.push(format!("− {}", stage_desc(s)));
        }
    }
    for s in &staged.stages {
        if !base.stages.iter().any(|x| x.num == s.num) {
            lines.push(format!("+ {}", stage_desc(s)));
        } else if let Some(b) = base.stages.iter().find(|x| x.num == s.num) {
            if b != s {
                lines.push(format!("~ {}", stage_desc(s)));
            }
        }
    }

    // New entries (by car not in base).
    for e in &staged.entries {
        if !base.entries.iter().any(|b| b.car == e.car) {
            let classes = if e.classes.is_empty() {
                String::new()
            } else {
                format!(", classes: {}", e.classes.join(", "))
            };
            let vehicle = match e.vehicle {
                Some(ref v) if !v.is_empty() => format!(", vehicle: {v}"),
                _ => String::new(),
            };
            lines.push(format!(
                "+ {} — new entry ({}, {}{})",
                e.name, e.car, classes, vehicle
            ));
        }
    }

    // Compare existing entries for field changes.
    for e in &staged.entries {
        if let Some(b) = base.entries.iter().find(|b| b.car == e.car) {
            if b.name != e.name {
                field_diff(&mut lines, &format!("{} name", e.car), &b.name, &e.name);
            }
            if b.classes != e.classes {
                let before = if b.classes.is_empty() {
                    "(none)".to_string()
                } else {
                    b.classes.join(", ")
                };
                let after = if e.classes.is_empty() {
                    "(none)".to_string()
                } else {
                    e.classes.join(", ")
                };
                lines.push(format!(
                    "~ {} classes: {} \u{2192} {}",
                    e.car, before, after
                ));
            }
            if b.vehicle != e.vehicle {
                field_diff(
                    &mut lines,
                    &format!("{} vehicle", e.car),
                    &opt_str(&b.vehicle),
                    &opt_str(&e.vehicle),
                );
            }
            if b.description != e.description {
                field_diff(
                    &mut lines,
                    &format!("{} description", e.car),
                    &desc_str(&b.description),
                    &desc_str(&e.description),
                );
            }
            if b.shared != e.shared {
                field_diff(
                    &mut lines,
                    &format!("{} shared car", e.car),
                    &shared_str(&b.shared),
                    &shared_str(&e.shared),
                );
            }
        }
    }

    // Removed entries (by car not in staged).
    for e in &base.entries {
        if !staged.entries.iter().any(|s| s.car == e.car) {
            lines.push(format!("\u{2212} {} — removed entry ({})", e.name, e.car));
        }
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(car: &str, name: &str) -> Entry {
        Entry::new(car, name)
    }

    fn delete(car: &str) -> EditOp {
        EditOp::Delete(car.to_string())
    }

    #[test]
    fn apply_ops_previews() {
        let a = entry("1", "A");
        let b = entry("2", "B");
        let current = vec![a, b];
        let ops = vec![
            EditOp::Upsert(entry("2", "B")),
            EditOp::Upsert(entry("3", "C")),
            delete("1"),
        ];
        let out = apply_ops(&current, &ops);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].car, "2");
        assert_eq!(out[1].car, "3");
    }

    #[test]
    fn compact_keeps_last_per_entry() {
        let current = vec![entry("7", "Alice")];
        let ops = vec![
            EditOp::Upsert(entry("8", "Bob")),
            EditOp::Upsert(entry("8", "Bob")),
            EditOp::Upsert(entry("8", "Robert")),
        ];
        let out = compact_ops(&ops, &current);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], EditOp::Upsert(entry("8", "Robert")));
    }

    #[test]
    fn compact_add_then_delete_collapses() {
        let current = vec![];
        let ops = vec![EditOp::Upsert(entry("9", "Dan")), delete("9")];
        assert!(compact_ops(&ops, &current).is_empty());
        // delete-then-re-add keeps the add
        let ops = vec![delete("9"), EditOp::Upsert(entry("9", "Dan"))];
        assert_eq!(
            compact_ops(&ops, &current),
            vec![EditOp::Upsert(entry("9", "Dan"))]
        );
    }

    #[test]
    fn compact_drops_noops() {
        let current = vec![entry("7", "Alice")];
        // class toggled off then on
        let mut toggled = current[0].clone();
        toggled.classes.retain(|c| c != "Outright");
        toggled.classes.push("Outright".into());
        assert_eq!(toggled, current[0]);
        let ops = vec![
            EditOp::Upsert(entry("7", "Alice")),
            EditOp::Upsert(toggled.clone()),
        ];
        assert!(compact_ops(&ops, &current).is_empty());
    }

    #[test]
    fn compact_delete_of_absent_entry_dropped() {
        let current = vec![entry("7", "Alice")];
        let ops = vec![delete("99")];
        assert!(compact_ops(&ops, &current).is_empty());
    }

    #[test]
    fn compact_preserves_order() {
        let current = vec![];
        let ops = vec![
            EditOp::Upsert(entry("2", "B")),
            EditOp::Upsert(entry("1", "A")),
            EditOp::Upsert(entry("3", "C")),
        ];
        let out = compact_ops(&ops, &current);
        assert_eq!(
            out,
            vec![
                EditOp::Upsert(entry("2", "B")),
                EditOp::Upsert(entry("1", "A")),
                EditOp::Upsert(entry("3", "C")),
            ]
        );
    }

    #[test]
    fn entry_diff_lines() {
        let current = vec![entry("7", "Alice"), entry("9", "Dan")];
        let mut updated = current[0].clone();
        updated.classes = vec!["Outright".into(), "Junior".into()];
        let ops = vec![
            EditOp::Upsert(updated),
            EditOp::Upsert(entry("8", "Bob")),
            delete("9"),
        ];
        let lines = entry_diff(&ops, &current);
        let joined = lines.join("\n");
        assert!(joined.contains("+ Bob \u{2014} new entry"));
        assert!(joined.contains("Dan"));
        assert!(joined.contains("classes: +Junior"));
    }

    #[test]
    fn entry_diff_reorder_summary() {
        // With order removed, no reorder detection is possible.
        // Same entries produce no diff.
        let current = vec![entry("1", "A"), entry("2", "B"), entry("3", "C")];
        let b = current[1].clone();
        let c = current[2].clone();
        let ops = vec![EditOp::Upsert(b), EditOp::Upsert(c)];
        let joined = entry_diff(&ops, &current).join("\n");
        assert!(joined.is_empty(), "{joined}");
    }

    fn base() -> EventInfo {
        EventInfo {
            id: "kt-2026-x".into(),
            sponsoring_club: "NDC".into(),
            year: "2026".into(),
            event_date: "2026-03-01".into(),
            ..Default::default()
        }
    }

    #[test]
    fn event_diff_fields_and_classes() {
        let mut staged = base();
        staged.sponsoring_club = "North District Club".into();
        staged.classes.push("Provisional".into());
        let lines = event_diff(&base(), &staged);
        let joined = lines.join("\n");
        assert!(joined.contains("Club / district: NDC → North District Club"));
        assert!(joined.contains("+ Classes: Provisional"));
    }

    #[test]
    fn event_diff_stages() {
        let mut base3 = base();
        base3.stages = (1..=3).map(crate::event::Stage::for_test).collect();
        let mut staged = base3.clone();
        staged.stages.pop();
        let lines = event_diff(&base3, &staged);
        let joined = lines.join("\n");
        assert!(joined.contains("− Test 3"));
        // no unrelated changes
        assert!(!joined.contains("Year"));
        assert!(event_diff(&base3, &base3).is_empty());
    }
}
