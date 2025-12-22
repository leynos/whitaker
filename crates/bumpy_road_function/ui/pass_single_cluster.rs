#![expect(dead_code, reason = "UI test fixture; functions are analysed but not invoked")]

fn is_valid_input(input: i32) -> bool {
    input > 0 && input < 100 && input != 5 && input != 7 && input != 9 && input != 11
}

pub fn mostly_linear(input: i32) -> i32 {
    let mut total = 0;

    if is_valid_input(input) {
        if input % 2 == 0 {
            total += 1;
        }
        total += 2;
    }

    total += input;
    total
}

fn dead_code_fixture_marker() {}

fn main() {}
