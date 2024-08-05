pub mod pmm;

use limine::request::HhdmRequest;

#[link_section = ".requests"]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();
