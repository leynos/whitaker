fn schedule(tasks: &[Task], budget: usize) -> Vec<Slot> {
    let mut slots = Vec::new();
    let mut remaining = budget;
    for task in tasks {
        if task.cost > remaining {
            break;
        }
        remaining -= task.cost;
        slots.push(Slot {
            name: task.name.clone(),
            cost: task.cost,
            queued: true,
        });
    }
    slots.sort_by_key(|slot| slot.cost);
    slots.dedup_by_key(|slot| slot.name.clone());
    if slots.is_empty() {
        return Vec::new();
    }
    slots.reverse();
    slots.truncate(budget);
    let chosen = slots.len();
    debug_assert!(chosen <= budget);
    slots
}
