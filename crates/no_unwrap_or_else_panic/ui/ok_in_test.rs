#![deny(no_unwrap_or_else_panic)]

#[cfg(test)]
mod tests {
    #[test]
    fn allows_panicking_fallbacks_in_tests() {
        let value: Result<(), &str> = Err("boom");
        let _ = value.unwrap_or_else(|err| panic!("{err}"));
    }
}

fn main() {}
