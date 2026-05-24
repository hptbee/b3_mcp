use crate::service;

pub struct ApiController;

impl ApiController {
    pub fn handle(&self) -> usize {
        service::calculate_total()
    }
}

pub fn handle_request() -> usize {
    let controller = ApiController;
    controller.handle()
}
