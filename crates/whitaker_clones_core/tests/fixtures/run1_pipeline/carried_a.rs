fn tally(entries: &[Entry], limit: usize) -> Tally {
    let mut sum = 0;
    let mut kept = 0;
    let mut widest = String::new();
    for entry in entries {
        if entry.weight == 0 {
            continue;
        }
        sum += entry.weight;
        kept += 1;
        if entry.label.len() > widest.len() {
            widest = entry.label.clone();
        }
    }
    if kept == 0 {
        return Tally::empty();
    }
    let mean = sum / kept;
    let spread = sum - mean;
    let label = widest.trim().to_lowercase();
    let flagged = mean > limit;
    Tally {
        sum,
        kept,
        mean,
        spread,
        widest: label,
        flagged,
        truncated: kept > limit,
    }
}

fn explain(tally: &Tally) -> String {
    let mut parts = Vec::new();
    parts.push(format!("sum={}", tally.sum));
    parts.push(format!("kept={}", tally.kept));
    if tally.flagged {
        parts.push("flagged".to_owned());
    }
    if tally.truncated {
        parts.push("truncated".to_owned());
    }
    parts.join(", ")
}
