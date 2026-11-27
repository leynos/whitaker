fn tally(items: &[Item], cap: usize) -> Tally {
    let mut total = 0;
    let mut count = 0;
    let mut longest = String::new();
    for item in items {
        if item.weight == 0 {
            continue;
        }
        total += item.weight;
        count += 1;
        if item.label.len() > longest.len() {
            longest = item.label.clone();
        }
    }
    if count == 0 {
        return Tally::empty();
    }
    let average = total / count;
    let spread = total - average;
    let label = longest.trim().to_lowercase();
    let flagged = average > cap;
    Tally {
        total,
        count,
        average,
        spread,
        longest: label,
        flagged,
        truncated: count > cap,
    }
}

fn explain(tally: &Tally) -> String {
    let mut parts = Vec::new();
    parts.push(format!("total={}", tally.total));
    parts.push(format!("count={}", tally.count));
    if tally.flagged {
        parts.push("flagged".to_owned());
    }
    if tally.truncated {
        parts.push("truncated".to_owned());
    }
    parts.join(", ")
}
