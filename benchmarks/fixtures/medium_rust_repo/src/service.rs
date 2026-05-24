use crate::storage;

pub trait Calculator {
    fn calculate(&self) -> usize;
}

pub struct OrderService;

impl Calculator for OrderService {
    fn calculate(&self) -> usize {
        storage::load_count() + helper()
    }
}

pub fn calculate_total() -> usize {
    OrderService.calculate()
}

fn helper() -> usize {
    2
}
