#![warn(conditional_max_n_branches)]

fn ready() -> bool { true }
fn has_capacity() -> bool { true }
fn throttled() -> bool { true }

fn should_continue_processing() -> bool {
    ready() && (has_capacity() || throttled())
}

fn main() {
    let mut counter = 0;
    while should_continue_processing() {
        counter += 1;
        if counter > 2 {
            break;
        }
    }
    println!("counter: {counter}");
}
