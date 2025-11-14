#![warn(conditional_max_n_branches)]

fn condition_a() -> bool { true }
fn condition_b() -> bool { true }
fn condition_c() -> bool { true }

fn main() {
    if condition_a() && condition_b() && condition_c() {
        println!("branches exceed limit");
    }
}
