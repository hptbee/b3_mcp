pub mod api;
pub mod service;
pub mod storage;

pub fn run() -> usize {
    api::handle_request()
}

#[cfg(test)]
mod tests {
    #[test]
    fn run_returns_value() {
        assert_eq!(super::run(), 3);
    }
}
