#![warn(conditional_max_n_branches)]

fn ready() -> bool { true }
fn has_capacity() -> bool { true }
fn throttled() -> bool { true }

fn main() {
    let mut counter = 0;
    while ready() && (has_capacity() || throttled()) {
        counter += 1;
        if counter > 2 {
            break;
        }
    }
    println!("counter: {counter}");
}
