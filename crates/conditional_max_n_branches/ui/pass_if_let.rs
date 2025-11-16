#![deny(conditional_max_n_branches)]

fn maybe_value() -> Option<i32> {
    Some(5)
}

fn main() {
    if let Some(value) = maybe_value() {
        println!("value: {value}");
    }
}
