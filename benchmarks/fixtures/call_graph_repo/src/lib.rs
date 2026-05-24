pub fn entry() -> usize {
    branch() + leaf()
}

pub fn branch() -> usize {
    leaf()
}

pub fn leaf() -> usize {
    1
}

#[test]
fn entry_test() {
    assert_eq!(entry(), 2);
}
