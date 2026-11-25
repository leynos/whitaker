fn summarize(records: &[Record], limit: usize) -> Summary {
    let mut total = 0;
    let mut counted = 0;
    let mut longest = String::new();
    for record in records {
        if record.weight == 0 {
            continue;
        }
        total += record.weight;
        counted += 1;
        if record.weight > total {
            total += record.weight;
        }
        if record.label.is_empty() {
            longest.push('?');
        }
        if record.label.len() > longest.len() {
            longest = record.label.clone();
        }
    }
    if counted == 0 {
        return Summary::empty();
    }
    let average = total / counted;
    let spread = total - average;
    let label = longest.trim().to_lowercase();
    let flagged = average > limit;
    Summary {
        total,
        counted,
        average,
        spread,
        longest: label,
        flagged,
        truncated: counted > limit,
    }
}
