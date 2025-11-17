#![deny(conditional_max_n_branches)]

fn is_ready() -> bool { true }
fn has_capacity() -> bool { true }

fn main() {
    if is_ready() && has_capacity() {
        println!("within limit");
    }
}
