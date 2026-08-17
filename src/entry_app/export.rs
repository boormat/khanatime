use super::types::{entry_sort_key, EntryEvent};

#[allow(dead_code)]
pub fn export_quick_add(event: &EntryEvent) -> String {
    let mut sorted = event.entries.clone();
    sorted.sort_by_key(entry_sort_key);
    let mut lines = Vec::new();
    for e in &sorted {
        if e.status == super::types::EntryStatus::Withdrawn {
            continue;
        }
        let car = if e.car.is_empty() {
            &e.preferred_car
        } else {
            &e.car
        };
        let classes = e.classes.join(" ");
        let vehicle = &e.vehicle;
        let shared = e.shared_car.as_deref().unwrap_or("");
        // Number Name  Class1 Class2  Vehicle  Description  SharedGroup
        lines.push(format!(
            "{} {}  {}  {}  {}",
            car, e.name, classes, vehicle, shared
        ));
    }
    lines.join("\n")
}
