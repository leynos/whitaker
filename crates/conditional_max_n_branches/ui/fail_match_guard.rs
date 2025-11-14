#![warn(conditional_max_n_branches)]

fn primary() -> bool { true }
fn secondary() -> bool { true }
fn tertiary() -> bool { true }

fn render(value: i32) {
    match value {
        other if primary() && secondary() && tertiary() => {
            println!("guard matched: {other}");
        }
        _ => {}
    }
}

fn main() {
    render(42);
}
