#![deny(string_continuation_style)]

macro_rules! expression_statement {
    ($value:literal) => {
        $value;
    };
}

fn main() {
    expression_statement!("alpha \
        beta");
}
