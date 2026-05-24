pub struct OrderRepository;

impl OrderRepository {
    pub fn count(&self) -> usize {
        1
    }
}

pub fn load_count() -> usize {
    let repository = OrderRepository;
    repository.count()
}
