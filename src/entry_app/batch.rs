use super::types::Entry;

#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum EditOp {
    Upsert(Entry),
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

pub fn entry_diff(ops: &[EditOp], current: &[Entry]) -> Vec<String> {
    let mut lines = Vec::new();
    for op in ops {
        match op {
            EditOp::Upsert(entry) => {
                if let Some(existing) = current.iter().find(|c| c.entry_no == entry.entry_no) {
                    let mut changes = Vec::new();
                    if existing.car != entry.car {
                        changes.push(format!("car: {} \u{2192} {}", existing.car, entry.car));
                    }
                    if existing.name != entry.name {
                        changes.push(format!("name: {} \u{2192} {}", existing.name, entry.name));
                    }
                    if existing.vehicle != entry.vehicle {
                        changes.push(format!(
                            "vehicle: {} \u{2192} {}",
                            existing.vehicle, entry.vehicle
                        ));
                    }
                    if existing.status != entry.status {
                        changes.push(format!(
                            "status: {} \u{2192} {}",
                            existing.status, entry.status
                        ));
                    }
                    if existing.classes != entry.classes {
                        changes.push(format!(
                            "classes: {:?} \u{2192} {:?}",
                            existing.classes, entry.classes
                        ));
                    }
                    if existing.order != entry.order {
                        changes.push(format!(
                            "order: {} \u{2192} {}",
                            existing.order, entry.order
                        ));
                    }
                    if existing.shared_car != entry.shared_car {
                        changes.push(format!(
                            "shared: {:?} \u{2192} {:?}",
                            existing.shared_car, entry.shared_car
                        ));
                    }
                    if !changes.is_empty() {
                        lines.push(format!("Update {}: {}", entry.car, changes.join(", ")));
                    }
                } else {
                    lines.push(format!("Add {} ({})", entry.name, entry.car));
                }
            }
            EditOp::Delete(no) => {
                if let Some(entry) = current.iter().find(|c| c.entry_no == *no) {
                    lines.push(format!("Delete {} ({})", entry.name, entry.car));
                }
            }
        }
    }
    lines
}
