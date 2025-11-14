fn alpha() -> bool { true }
fn beta() -> bool { true }
fn gamma() -> bool { true }

fn main() {
    if alpha() && beta() && gamma() {
        println!("custom limit allows three branches");
    }
}
