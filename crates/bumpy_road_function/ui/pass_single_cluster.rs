#![allow(dead_code)]

pub fn mostly_linear(input: i32) -> i32 {
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
    total
}

fn main() {}

