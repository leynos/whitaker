// run-rustfix
#![warn(string_continuation_style)]

fn main() {
    let value: u8 = 7;
    println!("alpha \
        beta{}", value);
}
