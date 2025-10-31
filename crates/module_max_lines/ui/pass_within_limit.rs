mod short_module {
    pub fn first() {}
    pub fn second() {}
}

fn main() {
    short_module::first();
    short_module::second();
}
