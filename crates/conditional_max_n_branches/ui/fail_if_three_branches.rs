#![warn(conditional_max_n_branches)]

fn condition_a() -> bool { true }
fn condition_b() -> bool { true }
fn condition_c() -> bool { true }

fn all_conditions_met() -> bool {
    if condition_a() && condition_b() && condition_c() {
        true
    } else {
        false
    }
}

fn main() {
    if all_conditions_met() {
        println!("branches exceed limit");
    }
}
