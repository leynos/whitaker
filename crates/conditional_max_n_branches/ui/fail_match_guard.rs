#![warn(conditional_max_n_branches)]

fn primary() -> bool { true }
fn secondary() -> bool { true }
fn tertiary() -> bool { true }

fn is_valid_for_rendering(_value: i32) -> bool {
    if primary() && secondary() && tertiary() {
        true
    } else {
        false
    }
}

fn render(value: i32) {
    match value {
        other if is_valid_for_rendering(other) => {
            println!("guard matched: {other}");
        }
        _ => {}
    }
}

fn main() {
    render(42);
}
