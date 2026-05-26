pub struct OrderService;

impl OrderService {
    pub fn create_order(&self, user_id: &str, sku: &str) -> String {
        format!("order:{user_id}:{sku}")
    }
}

pub fn order_creation_flow() {
    let service = OrderService;
    let _order_id = service.create_order("user-1", "sku-1");
}
