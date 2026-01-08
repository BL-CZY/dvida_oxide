use alloc::boxed::Box;
use ejcineque::wakers::{PRIMARY_IDE_WAKERS, SECONDARY_IDE_WAKERS};

use crate::crypto::binary_test;
use crate::drivers::ata::cmd;
use crate::drivers::ata::pata::{PATA_PRIMARY_BASE, PATA_SECONDARY_BASE};
use crate::hal::storage::IoErr;

use super::PataDevice;

const WAIT_TIME: u32 = 100000;
const WAIT_TICK_TIME: u32 = 10;
const SECTOR_SIZE: u16 = 512;

impl PataDevice {
    fn get_lba(&self, index: i64) -> u64 {
        let lba = if index < 0 {
            if self.lba48_supported {
                (self.lba28_sector_count - (index.abs() as u32)).into()
            } else {
                self.lba48_sector_count - (index.abs() as u64)
            }
        } else {
            index.try_into().unwrap()
        };

        // log!("get_lba: index={}, resolved_lba={}", index, lba);
        lba
    }

    fn verify_lba(&self, lba: u64, count: u16) -> Result<(), Box<dyn core::error::Error>> {
        // log!(
        //     "verify_lba: lba={}, count={}, lba48={}",
        //     lba,
        //     count,
        //     self.lba48_supported
        // );

        if self.lba48_supported {
            if lba + count as u64 > self.lba48_sector_count {
                // log!(
                //     "verify_lba: FAILED - LBA48 out of range (max={})",
                //     self.lba48_sector_count
                // );
                return Err(Box::new(IoErr::SectorOutOfRange));
            }
        } else {
            if lba + count as u64 > self.lba28_sector_count as u64 {
                // log!(
                //     "verify_lba: FAILED - LBA28 out of range (max={})",
                //     self.lba28_sector_count
                // );
                return Err(Box::new(IoErr::SectorOutOfRange));
            }
        }

        // log!("verify_lba: OK");
        Ok(())
    }

    fn wait_init(&mut self) -> Result<(), Box<dyn core::error::Error>> {
        // log!("wait_init: starting");
        let mut timer = 0;
        while binary_test(unsafe { self.status_port.read() } as u64, 7) {
            timer += 1;

            if timer > WAIT_TIME {
                // log!("wait_init: TIMEOUT after {} iterations", timer);
                return Err(Box::new(IoErr::InitTimeout));
            }
        }

        // log!("wait_init: completed after {} iterations", timer);
        Ok(())
    }

    fn io_init(&mut self, index: i64, count: u16) -> Result<u64, Box<dyn core::error::Error>> {
        // log!("io_init: index={}, count={}", index, count);

        if !self.identified {
            // log!("io_init: FAILED - device not identified");
            return Err(Box::new(IoErr::Unavailable));
        }

        let lba: u64 = self.get_lba(index);

        self.verify_lba(lba, count)?;

        self.wait_init()?;

        // log!("io_init: completed successfully, lba={}", lba);
        Ok(lba)
    }

    fn send_lba28(&mut self, count: u16, lba: u64) {
        // log!("send_lba28: count={}, lba={:#x}", count, lba);
        unsafe {
            self.drive_port
                .write(cmd::LBA28 | ((lba >> 24) | &0xFF) as u8);

            self.sector_count_port.write((count & 0xFF) as u8);
            self.lba_low_port.write((lba & 0xFF) as u8);
            self.lba_mid_port.write(((lba >> 8) & 0xFF) as u8);
            self.lba_mid_port.write(((lba >> 16) & 0xFF) as u8);
        }
    }

    fn send_read_lba28(&mut self, count: u16, lba: u64) {
        // log!("send_read_lba28: initiating read");
        self.send_lba28(count, lba);
        unsafe {
            self.cmd_port.write(cmd::READ_SECTORS);
        }
    }

    fn send_write_lba28(&mut self, count: u16, lba: u64) {
        // log!("send_write_lba28: initiating write");
        self.send_lba28(count, lba);
        unsafe {
            self.cmd_port.write(cmd::WRITE_SECTORS);
        }
    }

    fn send_lba48(&mut self, count: u16, lba: u64) {
        // log!("send_lba48: count={}, lba=0x{:#x}", count, lba);
        unsafe {
            self.drive_port.write(cmd::LBA48);

            self.sector_count_port.write(((count >> 8) & 0xFF) as u8);
            self.lba_low_port.write(((lba >> 24) & 0xFF) as u8);
            self.lba_mid_port.write(((lba >> 32) & 0xFF) as u8);
            self.lba_high_port.write(((lba >> 40) & 0xFF) as u8);

            self.sector_count_port.write((count & 0xFF) as u8);
            self.lba_low_port.write((lba & 0xFF) as u8);
            self.lba_mid_port.write(((lba >> 8) & 0xFF) as u8);
            self.lba_high_port.write(((lba >> 16) & 0xFF) as u8);
        }
    }

    fn send_read_lba48(&mut self, count: u16, lba: u64) {
        // log!("send_read_lba48: initiating read");
        self.send_lba48(count, lba);
        unsafe {
            self.cmd_port.write(cmd::READ_SECTORS_EXT);
        }
    }

    fn send_write_lba48(&mut self, count: u16, lba: u64) {
        // log!("send_write_lba48: initiating write");
        self.send_lba48(count, lba);
        unsafe {
            self.cmd_port.write(cmd::WRITE_SECTORS_EXT);
        }
    }

    fn wait_io(&mut self) -> Result<(), Box<dyn core::error::Error>> {
        for _ in 0..14 {
            unsafe {
                self.status_port.read();
            }
        }

        let mut timer = 0;
        while !binary_test(unsafe { self.status_port.read().into() }, 3)
            || binary_test(unsafe { self.status_port.read().into() }, 7)
        {
            timer += 1;
            if timer > WAIT_TIME {
                // log!("wait_io: TIMEOUT after {} iterations", timer);
                return Err(Box::new(IoErr::IOTimeout));
            }
        }
        Ok(())
    }

    fn wait_io_async_future(port: u16) -> WaitIOFuture {
        WaitIOFuture {
            port,
            is_done: false,
        }
    }

    async fn wait_io_async(&mut self) -> Result<(), Box<dyn core::error::Error>> {
        for _ in 0..14 {
            unsafe {
                self.status_port.read();
            }
        }

        let res = ejcineque::futures::race::race(
            ejcineque::time::wait(WAIT_TICK_TIME),
            Self::wait_io_async_future(self.port),
        )
        .await;

        match res {
            ejcineque::futures::race::Either::Left(_) => {
                // log!("wait_io_async: TIMEOUT");
                Err(Box::new(IoErr::IOTimeout))
            }
            ejcineque::futures::race::Either::Right(_) => Ok(()),
        }
    }

    fn read_data(
        &mut self,
        count: u16,
        result: &mut [u8],
    ) -> Result<(), Box<dyn core::error::Error>> {
        let bytes_needed = count as usize * 512;

        // log!(
        //     "read_data: reading {} sectors ({} bytes)",
        //     count,
        //     bytes_needed
        // );

        if result.len() < bytes_needed {
            // log!(
            //     "read_data: FAILED - buffer too small (need {}, have {})",
            //     bytes_needed,
            //     result.len()
            // );
            return Err(Box::new(IoErr::InputTooSmall));
        }

        for sector in 0..count {
            self.wait_io()?;

            // Calculate offset for this sector
            let offset = sector as usize * 512;

            // Read 256 words (512 bytes) for this sector
            for i in 0..256 {
                let word = unsafe { self.data_port.read() };
                let base = offset + i * 2;
                result[base] = (word & 0xFF) as u8;
                result[base + 1] = ((word >> 8) & 0xFF) as u8;
            }
        }

        // log!("read_data: successfully read {} sectors", count);
        Ok(())
    }

    async fn read_data_async(
        &mut self,
        count: u16,
        result: &mut [u8],
    ) -> Result<(), Box<dyn core::error::Error>> {
        let bytes_needed = count as usize * 512;

        if result.len() < bytes_needed {
            // log!(
            //     "read_data_async: FAILED - buffer too small (need {}, have {})",
            //     bytes_needed,
            //     result.len()
            // );
            return Err(Box::new(IoErr::InputTooSmall));
        }

        // log!(
        //     "read_data_async: prepared to read {} sectors ({} bytes)",
        //     count,
        //     bytes_needed
        // );

        for sector in 0..count {
            self.wait_io_async().await?;
            // log!("read_data_async: reading sector {}/{}", sector + 1, count);
            self.wait_io()?;

            // Calculate offset for this sector
            let offset = sector as usize * 512;

            // Read 256 words (512 bytes) for this sector
            for i in 0..256 {
                let word = unsafe { self.data_port.read() };
                let base = offset + i * 2;
                result[base] = (word & 0xFF) as u8;
                result[base + 1] = ((word >> 8) & 0xFF) as u8;
            }
        }

        // log!("read_data_async: successfully read {} sectors", count);
        Ok(())
    }

    fn flush_cache(&mut self) -> Result<(), Box<dyn core::error::Error>> {
        // log!("flush_cache: flushing drive cache");
        unsafe {
            self.cmd_port.write(cmd::FLUSH_CACHE);
        }

        self.wait_init()?;

        // log!("flush_cache: completed");
        Ok(())
    }

    fn write_data(&mut self, count: u16, input: &[u8]) -> Result<(), Box<dyn core::error::Error>> {
        // log!(
        //     "write_data: writing {} sectors ({} bytes)",
        //     count,
        //     count as usize * 512
        // );

        for sector in 0..count as usize {
            self.wait_io()?;

            for byte in 0..256usize {
                unsafe {
                    self.data_port.write(
                        (input[sector * 512 + (byte * 2) + 1] as u16) << 8
                            | input[sector * 512 + byte * 2] as u16,
                    );
                }
            }
        }

        // log!("write_data: successfully wrote {} sectors", count);
        Ok(())
    }

    async fn write_data_async(
        &mut self,
        count: u16,
        input: &[u8],
    ) -> Result<(), Box<dyn core::error::Error>> {
        // log!(
        //     "write_data_async: writing {} sectors ({} bytes)",
        //     count,
        //     count as usize * 512
        // );

        for sector in 0..count as usize {
            self.wait_io_async().await?;
            self.wait_io()?;
            // log!("write_data_async: writing sector {}/{}", sector + 1, count);

            for byte in 0..256usize {
                unsafe {
                    self.data_port.write(
                        (input[sector * 512 + (byte * 2) + 1] as u16) << 8
                            | input[sector * 512 + byte * 2] as u16,
                    );
                }
            }
        }

        // log!("write_data_async: successfully wrote {} sectors", count);
        Ok(())
    }

    pub fn pio_read_sectors(
        &mut self,
        index: i64,
        count: u16,
        output: &mut [u8],
    ) -> Result<(), Box<dyn core::error::Error>> {
        // log!(
        //     "pio_read_sectors: starting read at index={}, count={}",
        //     index,
        //     count
        // );

        let lba = match self.io_init(index, count) {
            Ok(val) => val,
            Err(e) => {
                // log!("pio_read_sectors: FAILED during io_init");
                return Err(e);
            }
        };

        if self.lba48_supported {
            self.send_read_lba48(count, lba);
        } else {
            self.send_read_lba28(count, lba);
        }

        self.read_data(count, output)?;

        // log!("pio_read_sectors: completed successfully");
        Ok(())
    }

    pub async fn pio_read_sectors_async(
        &mut self,
        index: i64,
        count: u16,
        output: &mut [u8],
    ) -> Result<(), Box<dyn core::error::Error>> {
        // log!(
        //     "pio_read_sectors_async: starting read at index={}, count={}",
        //     index,
        //     count
        // );

        let lba = match self.io_init(index, count) {
            Ok(val) => val,
            Err(e) => {
                // log!("pio_read_sectors_async: FAILED during io_init");
                return Err(e);
            }
        };

        if self.lba48_supported {
            self.send_read_lba48(count, lba);
        } else {
            self.send_read_lba28(count, lba);
        }

        self.read_data_async(count, output).await?;

        // log!("pio_read_sectors_async: completed successfully");
        Ok(())
    }

    pub fn pio_write_sectors(
        &mut self,
        index: i64,
        count: u16,
        input: &[u8],
    ) -> Result<(), Box<dyn core::error::Error>> {
        // log!(
        //     "pio_write_sectors: starting write at index={}, count={}",
        //     index,
        //     count
        // );

        if input.len() < (count * SECTOR_SIZE).into() {
            // log!(
            //     "pio_write_sectors: FAILED - input too small (need {}, have {})",
            //     count * SECTOR_SIZE,
            //     input.len()
            // );
            return Err(Box::new(IoErr::InputTooSmall));
        }

        let lba = match self.io_init(index, count) {
            Ok(val) => val,
            Err(e) => {
                // log!("pio_write_sectors: FAILED during io_init");
                return Err(e);
            }
        };

        if self.lba48_supported {
            self.send_write_lba48(count, lba);
        } else {
            self.send_write_lba28(count, lba);
        }

        self.write_data(count, input)?;

        self.flush_cache()?;

        // log!("pio_write_sectors: completed successfully");
        Ok(())
    }

    pub async fn pio_write_sectors_async(
        &mut self,
        index: i64,
        count: u16,
        input: &[u8],
    ) -> Result<(), Box<dyn core::error::Error>> {
        // log!(
        //     "pio_write_sectors_async: starting write at index={}, count={}",
        //     index,
        //     count
        // );

        if input.len() < (count * SECTOR_SIZE).into() {
            // log!(
            //     "pio_write_sectors_async: FAILED - input too small (need {}, have {})",
            //     count * SECTOR_SIZE,
            //     input.len()
            // );
            return Err(Box::new(IoErr::InputTooSmall));
        }

        let lba = match self.io_init(index, count) {
            Ok(val) => val,
            Err(e) => {
                // log!("pio_write_sectors_async: FAILED during io_init");
                return Err(e);
            }
        };

        if self.lba48_supported {
            self.send_write_lba48(count, lba);
        } else {
            self.send_write_lba28(count, lba);
        }

        self.write_data_async(count, input).await?;

        self.flush_cache()?;

        // log!("pio_write_sectors_async: completed successfully");
        Ok(())
    }
}

pub struct WaitIOFuture {
    is_done: bool,
    port: u16,
}

impl Future for WaitIOFuture {
    type Output = ();

    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        if self.is_done {
            return core::task::Poll::Ready(());
        }

        self.is_done = true;

        if self.port == PATA_PRIMARY_BASE {
            x86_64::instructions::interrupts::without_interrupts(|| {
                PRIMARY_IDE_WAKERS.lock().push(cx.waker().clone());
            });
        } else if self.port == PATA_SECONDARY_BASE {
            x86_64::instructions::interrupts::without_interrupts(|| {
                SECONDARY_IDE_WAKERS.lock().push(cx.waker().clone());
            });
        } else {
            // log!("WaitIOFuture::poll: PANIC - invalid port {:#x}", self.port);
            panic!("Drive doesn't exist");
        }

        core::task::Poll::Pending
    }
}
