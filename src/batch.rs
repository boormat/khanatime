//! Staged-edit batching for officials.
//!
//! While in edit/admin mode, edits are queued locally (staged) instead of
//! being sent.  On Save the batch is compacted to remove redundant edits,
//! diffed against the committed state for confirmation, then emitted as entry
//! messages (or one setup manifest, for the Event Admin page).  Pure + testable.

use crate::event::{Entry, EntryStatus, EventInfo, Stage, TimingStyle};

/// A staged entry edit (admin edit mode on the Entries page).
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum EditOp {
    /// Insert or replace the entry (keyed by entry number).
    Upsert(Entry),
    /// Tombstone: remove the entry by entry number.
    Delete(u32),
}

fn op_key(op: &EditOp) -> u32 {
    match op {
        EditOp::Upsert(e) => e.entry_no,
        EditOp::Delete(no) => *no,
    }
}

fn upsert(entries: &mut Vec<Entry>, entry: Entry) {
    if let Some(existing) = entries.iter_mut().find(|e| e.entry_no == entry.entry_no) {
        *existing = entry;
    } else {
        entries.push(entry);
    }
}

/// Fold staged ops over the committed entry list (WYSIWYG preview while
/// editing).
pub fn apply_ops(entries: &[Entry], ops: &[EditOp]) -> Vec<Entry> {
    let mut out: Vec<Entry> = entries.to_vec();
    for op in ops {
        match op {
            EditOp::Upsert(e) => upsert(&mut out, e.clone()),
            EditOp::Delete(no) => out.retain(|e| e.entry_no != *no),
        }
    }
    out
}

/// Collapse a staged batch into the minimal set of messages to send:
/// keep only the last op per entry (each upsert is a full snapshot, so the
/// last one fully determines that entry's final state), then drop no-ops
/// against the committed state (an upsert already present, or a tombstone for
/// an entry that isn't committed).  Order is stable, first-seen.
pub fn compact_ops(ops: &[EditOp], current: &[Entry]) -> Vec<EditOp> {
    let mut kept: Vec<EditOp> = Vec::new();
    for op in ops {
        kept.retain(|o| op_key(o) != op_key(op));
        kept.push(op.clone());
    }
    kept.retain(|op| match op {
        EditOp::Upsert(e) => !current.iter().any(|c| c.entry_no == e.entry_no && c == e),
        EditOp::Delete(no) => current.iter().any(|c| c.entry_no == *no),
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

fn status_str(s: &EntryStatus) -> String {
    s.to_string()
}

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

/// Human-readable list of changes a batch of entry ops makes, for the confirm
/// dialog.  Runs over the already-compacted ops.
pub fn entry_diff(ops: &[EditOp], current: &[Entry]) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let order_changed = |e: &Entry| -> bool {
        current
            .iter()
            .find(|c| c.entry_no == e.entry_no)
            .map(|c| c.order != e.order)
            .unwrap_or(false)
    };
    for op in ops {
        match op {
            EditOp::Upsert(e) => match current.iter().find(|c| c.entry_no == e.entry_no) {
                None => lines.push(format!(
                    "+ {} — new entry (preferred {}, number {})",
                    e.name,
                    if e.preferred_car.is_empty() {
                        "none"
                    } else {
                        e.preferred_car.as_str()
                    },
                    car_or_unassigned(&e.car)
                )),
                Some(cur) => {
                    if cur.car != e.car {
                        lines.push(format!(
                            "~ {} — number: {} → {}",
                            e.name,
                            car_or_unassigned(&cur.car),
                            car_or_unassigned(&e.car)
                        ));
                    }
                    if cur.preferred_car != e.preferred_car {
                        lines.push(format!(
                            "~ {} — preferred: {} → {}",
                            e.name,
                            if cur.preferred_car.is_empty() {
                                "(none)"
                            } else {
                                cur.preferred_car.as_str()
                            },
                            if e.preferred_car.is_empty() {
                                "(none)"
                            } else {
                                e.preferred_car.as_str()
                            }
                        ));
                    }
                    if cur.shared_car != e.shared_car {
                        lines.push(format!(
                            "~ {} — shared car: {} → {}",
                            e.name,
                            shared_str(&cur.shared_car),
                            shared_str(&e.shared_car)
                        ));
                    }
                    if cur.name != e.name || cur.vehicle != e.vehicle {
                        lines.push(format!(
                            "~ Car {} {} — {}",
                            e.car,
                            e.name,
                            if cur.name != e.name {
                                format!("name: {} → {}", cur.name, e.name)
                            } else {
                                format!("vehicle: {} → {}", cur.vehicle, e.vehicle)
                            }
                        ));
                    }
                    if cur.status != e.status {
                        lines.push(format!(
                            "~ Car {} — status: {} → {}",
                            e.car,
                            status_str(&cur.status),
                            status_str(&e.status)
                        ));
                    }
                    let (added, removed) = class_delta(&cur.classes, &e.classes);
                    if !removed.is_empty() {
                        lines.push(format!(
                            "~ Car {} — classes: −{}",
                            e.car,
                            removed.join(", ")
                        ));
                    }
                    if !added.is_empty() {
                        lines.push(format!("~ Car {} — classes: +{}", e.car, added.join(", ")));
                    }
                }
            },
            EditOp::Delete(no) => {
                if let Some(e) = current.iter().find(|c| c.entry_no == *no) {
                    lines.push(format!(
                        "− Car {} {} (removed)",
                        car_or_unassigned(&e.car),
                        e.name
                    ));
                }
            }
        }
    }
    // Single summary line for pure reorders (individually noisy).
    let reordered: Vec<&str> = ops
        .iter()
        .filter_map(|op| match op {
            EditOp::Upsert(e) if order_changed(e) => Some(e.name.as_str()),
            _ => None,
        })
        .collect();
    if !reordered.is_empty() {
        lines.push(format!("↕ Reordered: {}", reordered.join(", ")));
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
        "Test {}: {} — best {} of {}, {}",
        s.num, s.name, s.best_x, s.repeats, style
    )
}

/// Human-readable list of changes from one event config to another, for the
/// Event Admin confirm dialog.
pub fn event_diff(base: &EventInfo, staged: &EventInfo) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut field = |label: &str, a: &str, b: &str| {
        if a != b {
            lines.push(format!("~ {label}: {} → {}", show(a), show(b)));
        }
    };
    field(
        "Club / district",
        &base.sponsoring_club,
        &staged.sponsoring_club,
    );
    field("Name", &base.name, &staged.name);
    field("Year", &base.year, &staged.year);
    field("Event date", &base.event_date, &staged.event_date);
    field("Entry open", &base.entry_open, &staged.entry_open);
    field("Entry close", &base.entry_close, &staged.entry_close);
    field("Stripe link", &base.stripe_link, &staged.stripe_link);
    field("Parent room", &base.parent_room, &staged.parent_room);
    field("Homeserver", &base.homeserver, &staged.homeserver);
    field("Element link", &base.element_link, &staged.element_link);
    if base.entries_enabled != staged.entries_enabled {
        lines.push(if staged.entries_enabled {
            "+ In-app entries: enabled".to_string()
        } else {
            "− In-app entries: disabled".to_string()
        });
    }

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
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    fn entry(car: &str, name: &str) -> Entry {
        let mut e = Entry::new(car, name);
        e.entry_no = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        e
    }

    fn with_no(mut e: Entry, no: u32) -> Entry {
        e.entry_no = no;
        e
    }

    fn with_status(mut e: Entry, status: EntryStatus) -> Entry {
        e.status = status;
        e
    }

    fn delete(no: u32) -> EditOp {
        EditOp::Delete(no)
    }

    #[test]
    fn apply_ops_previews() {
        let a = with_no(entry("1", "A"), 1);
        let b = with_no(entry("2", "B"), 2);
        let current = vec![a, b];
        let ops = vec![
            EditOp::Upsert(with_status(
                with_no(entry("2", "B"), 2),
                EntryStatus::Confirmed,
            )),
            EditOp::Upsert(with_no(entry("3", "C"), 3)),
            delete(1),
        ];
        let out = apply_ops(&current, &ops);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].entry_no, 2);
        assert_eq!(out[0].status, EntryStatus::Confirmed);
        assert_eq!(out[1].entry_no, 3);
    }

    #[test]
    fn compact_keeps_last_per_entry() {
        let current = vec![with_no(entry("7", "Alice"), 1)];
        let ops = vec![
            EditOp::Upsert(with_no(entry("8", "Bob"), 2)),
            EditOp::Upsert(with_no(entry("8", "Bob"), 2)),
            EditOp::Upsert(with_no(entry("8", "Robert"), 2)),
        ];
        let out = compact_ops(&ops, &current);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], EditOp::Upsert(with_no(entry("8", "Robert"), 2)));
    }

    #[test]
    fn compact_add_then_delete_collapses() {
        let current = vec![];
        let ops = vec![EditOp::Upsert(with_no(entry("9", "Dan"), 1)), delete(1)];
        assert!(compact_ops(&ops, &current).is_empty());
        // delete-then-re-add keeps the add
        let ops = vec![delete(1), EditOp::Upsert(with_no(entry("9", "Dan"), 1))];
        assert_eq!(
            compact_ops(&ops, &current),
            vec![EditOp::Upsert(with_no(entry("9", "Dan"), 1))]
        );
    }

    #[test]
    fn compact_drops_noops() {
        let current = vec![with_status(
            with_no(entry("7", "Alice"), 1),
            EntryStatus::Confirmed,
        )];
        // class toggled off then on, status changed then reverted
        let mut toggled = current[0].clone();
        toggled.classes.retain(|c| c != "Outright");
        toggled.classes.push("Outright".into());
        assert_eq!(toggled, current[0]);
        let ops = vec![
            EditOp::Upsert(with_status(
                with_no(entry("7", "Alice"), 1),
                EntryStatus::Submitted,
            )),
            EditOp::Upsert(toggled.clone()),
        ];
        assert!(compact_ops(&ops, &current).is_empty());
    }

    #[test]
    fn compact_delete_of_absent_entry_dropped() {
        let current = vec![with_no(entry("7", "Alice"), 1)];
        let ops = vec![delete(99)];
        assert!(compact_ops(&ops, &current).is_empty());
    }

    #[test]
    fn compact_preserves_order() {
        let current = vec![];
        let ops = vec![
            EditOp::Upsert(with_no(entry("2", "B"), 2)),
            EditOp::Upsert(with_no(entry("1", "A"), 1)),
            EditOp::Upsert(with_no(entry("3", "C"), 3)),
        ];
        let out = compact_ops(&ops, &current);
        assert_eq!(
            out,
            vec![
                EditOp::Upsert(with_no(entry("2", "B"), 2)),
                EditOp::Upsert(with_no(entry("1", "A"), 1)),
                EditOp::Upsert(with_no(entry("3", "C"), 3)),
            ]
        );
    }

    #[test]
    fn entry_diff_lines() {
        let current = vec![
            with_status(with_no(entry("7", "Alice"), 1), EntryStatus::Submitted),
            with_no(entry("9", "Dan"), 2),
        ];
        let mut updated = current[0].clone();
        updated.status = EntryStatus::Confirmed;
        updated.classes = vec!["Outright".into(), "Junior".into()];
        updated.car = "77".into();
        let ops = vec![
            EditOp::Upsert(updated),
            EditOp::Upsert(with_no(entry("8", "Bob"), 3)),
            delete(2),
        ];
        let lines = entry_diff(&ops, &current);
        let joined = lines.join("\n");
        assert!(joined.contains("+ Bob — new entry")); // renumbered/shifted
        assert!(joined.contains("− Car 9 Dan (removed)"));
        assert!(joined.contains("number: 7 → 77"));
        assert!(joined.contains("status: entry submitted → confirmed"));
        assert!(joined.contains("classes: +Junior"));
    }

    #[test]
    fn entry_diff_preferred_and_shared_lines() {
        let current = vec![with_no(entry("7", "Alice"), 1)];
        let mut updated = current[0].clone();
        updated.preferred_car = "55".into();
        updated.shared_car = Some("ABC123".into());
        let ops = vec![EditOp::Upsert(updated)];
        let joined = entry_diff(&ops, &current).join("\n");
        assert!(joined.contains("preferred: (none) → 55"));
        assert!(joined.contains("shared car: (none) → ABC123"));
    }

    #[test]
    fn entry_diff_reorder_summary() {
        let current = vec![
            with_no(entry("1", "A"), 1),
            with_no(entry("2", "B"), 2),
            with_no(entry("3", "C"), 3),
        ];
        let mut b = current[1].clone();
        b.order = 10;
        let mut c = current[2].clone();
        c.order = 20;
        let ops = vec![EditOp::Upsert(b), EditOp::Upsert(c)];
        let joined = entry_diff(&ops, &current).join("\n");
        assert!(joined.contains("↕ Reordered: B, C"), "{joined}");
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
