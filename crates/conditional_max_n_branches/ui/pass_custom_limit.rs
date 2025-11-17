fn alpha() -> bool { true }
fn beta() -> bool { true }
fn gamma() -> bool { true }

fn all_conditions_satisfied() -> bool {
    matches!((alpha(), beta(), gamma()), (true, true, true))
}

fn main() {
    if all_conditions_satisfied() {
        println!("custom limit allows three branches");
    }
}
