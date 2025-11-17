fn ready() -> bool { true }
fn approved() -> bool { true }

fn main() {
    if ready() && approved() {
        println!("overrides should warn");
    }
}
