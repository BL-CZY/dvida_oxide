use x86_64::VirtAddr;

#[derive(Debug)]
pub struct SataAhci {}

impl SataAhci {
    pub fn new(base: VirtAddr) -> Self {
        Self {}
    }
}

