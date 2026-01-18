use x86_64::VirtAddr;

use crate::hal::storage::HalBlockDevice;

#[derive(Debug)]
pub struct SataAhci {}

impl SataAhci {
    pub fn new(base: VirtAddr) -> Self {
        Self {}
    }
}

impl HalBlockDevice for SataAhci {
    fn sector_count(&mut self) -> u64 {
        todo!()
    }
    fn sectors_per_track(&mut self) -> u16 {
        todo!()
    }

    fn init(&mut self) -> Result<(), alloc::boxed::Box<dyn core::error::Error + Send + Sync>> {
        todo!()
    }

    fn read_sectors_async(
        &mut self,
        index: i64,
        count: u16,
        output: &mut [u8],
    ) -> core::pin::Pin<
        alloc::boxed::Box<
            dyn Future<Output = Result<(), alloc::boxed::Box<dyn core::error::Error + Send + Sync>>>
                + Send
                + Sync,
        >,
    > {
        todo!()
    }

    fn write_sectors_async(
        &mut self,
        index: i64,
        count: u16,
        input: &[u8],
    ) -> core::pin::Pin<
        alloc::boxed::Box<
            dyn Future<Output = Result<(), alloc::boxed::Box<dyn core::error::Error + Send + Sync>>>
                + Send
                + Sync,
        >,
    > {
        todo!()
    }
}
