#![expect(dead_code, reason = "UI test fixture; functions are analysed but not invoked")]

pub fn bumpy(input: i32) -> i32 {
    let mut total = 0;

    if input > 0
        && input < 100
        && input != 5
        && input != 7
        && input != 9
        && input != 11
    {
        if input % 2 == 0 {
            total += 1;
        }
        total += 2;
    }

    total += input;

    if input > 1000
        && input < 2000
        && input != 1500
        && input != 1750
        && input != 1800
        && input != 1900
    {
        if input % 3 == 0 {
            total += 3;
        }
        total += 4;
    }

    total
}

fn dead_code_fixture_marker() {}

fn main() {}
