use alloc::vec;
use alloc::vec::Vec;

use crate::drivers::ata::cmd;
use crate::utils::binary_test;

use super::PataDevice;

#[derive(Debug)]
pub enum PataPioIoErr {
    DeviceUnidentified,
    SectorOutOfRange,
    InitTimeout,
    IOTimeout,
    FlushCacheTimeout,
    InputTooSmall,
}

const WAIT_TIME: u32 = 100000;

impl PataDevice {
    fn get_lba(&self, index: i64) -> u64 {
        if index < 0 {
            if self.lba48_supported {
                (self.lba28_sector_count - (index.abs() as u32)).into()
            } else {
                self.lba48_sector_count - (index.abs() as u64)
            }
        } else {
            // dosn't matter as index is guaranteed to be bigger than 0
            index.try_into().unwrap()
        }
    }

    fn verify_lba(&self, lba: u64, count: u16) -> Result<(), PataPioIoErr> {
        if self.lba48_supported {
            if lba + count as u64 > self.lba48_sector_count {
                return Err(PataPioIoErr::SectorOutOfRange);
            }
        } else {
            if lba + count as u64 > self.lba28_sector_count as u64 {
                return Err(PataPioIoErr::SectorOutOfRange);
            }
        }

        Ok(())
    }

    fn wait_init(&mut self) -> Result<(), PataPioIoErr> {
        let mut timer = 0;
        while binary_test(unsafe { self.status_port.read() } as u64, 7) {
            timer += 1;

            if timer > WAIT_TIME {
                return Err(PataPioIoErr::InitTimeout);
            }
        }

        Ok(())
    }

    fn io_init(&mut self, index: i64, count: u16) -> Result<u64, PataPioIoErr> {
        if !self.identified {
            return Err(PataPioIoErr::DeviceUnidentified);
        }

        let lba: u64 = self.get_lba(index);

        if let Err(e) = self.verify_lba(lba, count) {
            return Err(e);
        }

        if let Err(e) = self.wait_init() {
            return Err(e);
        }

        Ok(lba)
    }

    fn send_lba28(&mut self, count: u16, lba: u64) {
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
        self.send_lba28(count, lba);
        unsafe {
            self.cmd_port.write(cmd::READ_SECTORS);
        }
    }

    fn send_write_lba28(&mut self, count: u16, lba: u64) {
        self.send_lba28(count, lba);
        unsafe {
            self.cmd_port.write(cmd::WRITE_SECTORS);
        }
    }

    fn send_lba48(&mut self, count: u16, lba: u64) {
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
        self.send_lba48(count, lba);
        unsafe {
            self.cmd_port.write(cmd::READ_SECTORS_EXT);
        }
    }

    fn send_write_lba48(&mut self, count: u16, lba: u64) {
        self.send_lba48(count, lba);
        unsafe {
            self.cmd_port.write(cmd::WRITE_SECTORS_EXT);
        }
    }

    fn wait_io(&mut self) -> Result<(), PataPioIoErr> {
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
                return Err(PataPioIoErr::IOTimeout);
            }
        }
        Ok(())
    }

    fn read_data(&mut self, count: u16, result: &mut Vec<u8>) -> Result<(), PataPioIoErr> {
        for _ in 0..count {
            if let Err(e) = self.wait_io() {
                return Err(e);
            }

            for _ in 0..256 {
                let temp = unsafe { self.data_port.read() };
                result.push((temp & 0xFF) as u8);
                result.push(((temp >> 8) & 0xFF) as u8);
            }
        }

        Ok(())
    }

    fn flush_cache(&mut self) -> Result<(), PataPioIoErr> {
        unsafe {
            self.cmd_port.write(cmd::FLUSH_CACHE);
        }

        if let Err(_) = self.wait_init() {
            return Err(PataPioIoErr::FlushCacheTimeout);
        }

        Ok(())
    }

    fn write_data(&mut self, count: u16, input: &mut Vec<u8>) -> Result<(), PataPioIoErr> {
        for sector in 0..count as usize {
            if let Err(e) = self.wait_io() {
                return Err(e);
            }

            for byte in 0..256usize {
                unsafe {
                    self.data_port.write(
                        (input[sector * 512 + (byte * 2) + 1] as u16) << 8
                            | input[sector * 512 + byte * 2] as u16,
                    );
                }
            }
        }

        Ok(())
    }

    pub fn pio_read_sectors(&mut self, index: i64, count: u16) -> Result<Vec<u8>, PataPioIoErr> {
        let lba = match self.io_init(index, count) {
            Ok(val) => val,
            Err(e) => return Err(e),
        };

        if self.lba48_supported {
            self.send_read_lba48(count, lba);
        } else {
            self.send_read_lba28(count, lba);
        }

        let mut result: Vec<u8> = vec![];

        if let Err(e) = self.read_data(count, &mut result) {
            return Err(e);
        }

        Ok(result)
    }

    pub fn pio_write_sectors(
        &mut self,
        index: i64,
        count: u16,
        input: &mut Vec<u8>,
    ) -> Result<(), PataPioIoErr> {
        if input.len() < (count * 512).into() {
            return Err(PataPioIoErr::InputTooSmall);
        }

        let lba = match self.io_init(index, count) {
            Ok(val) => val,
            Err(e) => return Err(e),
        };

        if self.lba48_supported {
            self.send_write_lba48(count, lba);
        } else {
            self.send_write_lba28(count, lba);
        }

        if let Err(e) = self.write_data(count, input) {
            return Err(e);
        }

        if let Err(e) = self.flush_cache() {
            return Err(e);
        }

        Ok(())
    }
}
