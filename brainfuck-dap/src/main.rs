use dap::DapService;

use crate::dap::io_thread;

mod dap;

struct Data {}

impl Data {
    fn init(&mut self, i: i32) -> i32 {
        1
    }
}

fn main() {
    let mut dap_service = DapService::new(Data {})
        .register("init".to_string(), Box::new(Data::init))
        .build();
    dap_service.start();
}
