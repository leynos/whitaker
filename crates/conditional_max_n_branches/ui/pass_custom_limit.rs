#![warn(conditional_max_n_branches)]

fn alpha() -> bool { true }
fn beta() -> bool { true }
fn gamma() -> bool { true }

fn all_conditions_satisfied() -> bool {
    alpha() && beta() && gamma()
}

fn main() {
    if all_conditions_satisfied() {
        println!("custom limit allows three branches");
    }
}
