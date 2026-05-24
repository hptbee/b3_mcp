pub fn a() -> usize {
    b()
}

pub fn b() -> usize {
    c()
}

pub fn c() -> usize {
    a()
}
