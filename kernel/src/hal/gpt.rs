use core::ops::Deref;

use crate::ejcineque::pools::{DISK_IO_BUFFER_POOL_SECTOR_SIZE, DiskIOBufferPoolHandle};
use crate::hal::buffer::Buffer;
use crate::{hal, log};
use alloc::boxed::Box;
use alloc::string::{FromUtf16Error, String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use bytemuck::{Pod, Zeroable};
use thiserror::Error;

use crate::crypto;
use crate::crypto::guid::Guid;

#[derive(Pod, Zeroable, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(C, packed)]
pub struct GPTHeader {
    sig: [u8; 8],
    revision: u32,
    size: u32,
    header_crc32: u32,
    reserved: u32,
    loc: u64,
    backup_loc: u64,
    first_usable_block: u64,
    last_usable_block: u64,
    guid: u128,
    array_start: u64,
    entry_num: u32,
    entry_size: u32,
    array_crc32: u32,
}

impl GPTHeader {
    pub fn guid(&self) -> Guid {
        Guid::from_u128(self.guid)
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Copy, Pod, Zeroable, Default)]
#[repr(C, packed)]
pub struct GPTEntry {
    type_guid: u128,
    unique_guid: u128,
    pub start_lba: u64,
    pub end_lba: u64,
    pub flags: u64,
    name1: [u16; 32],
    name2: [u16; 4],
}

impl GPTEntry {
    pub fn is_empty(&self) -> bool {
        self.start_lba == 0
    }

    pub fn type_guid(&self) -> Guid {
        Guid::from_u128(self.type_guid)
    }

    pub fn unique_guid(&self) -> Guid {
        Guid::from_u128(self.unique_guid)
    }

    pub fn get_name(&self) -> String {
        self.name1
            .into_iter()
            .chain(self.name2)
            .filter(|&c| c != 0)
            .map(|c| char::from_u32(c as u32).unwrap_or(' '))
            .collect()
    }
}

#[derive(Debug, Error)]
pub enum GPTErr {
    #[error("The buffer input is too small")]
    BufferTooSmall,
    #[error("A GPT table already exists")]
    GPTAlreadyExist,
    #[error("A GPT table doesn't exist")]
    GPTNonExist,
    #[error("The GPT table is corrupted")]
    GPTCorrupted,
    #[error("The Array size is bad")]
    BadArrayEntrySize,
    #[error("There is no free slot")]
    NoFreeSlot,
    #[error("Partition overlapped")]
    OverlappingPartition,
    #[error("LBA range is invalid")]
    InvalidLBARange,
    #[error("Entry index is invalid")]
    InvalidEntryIndex,
    #[error("Entry is already empty")]
    EntryAlreadyEmpty,
    #[error("The name is too long")]
    NameTooLong,
    #[error("Failed to serialize {0}")]
    SerializationErr(FromUtf16Error),
    #[error("Read/Write failed: {0}")]
    Io(String),
    #[error("Drive didn't respond")]
    DriveDidntRespond,
}

pub struct GptReader {
    idx: usize,
}

pub const SECTOR_SIZE: usize = 512;

impl GptReader {
    pub fn get_buffer() -> DiskIOBufferPoolHandle<SECTOR_SIZE> {
        DISK_IO_BUFFER_POOL_SECTOR_SIZE.get_buffer()
    }

    pub fn new(idx: usize) -> Self {
        Self { idx }
    }

    async fn read_sectors_async(
        &self,
        lba: i64,
        buf: Buffer,
    ) -> Result<(), Box<dyn core::error::Error + Send + Sync>> {
        Ok(hal::storage::read_sectors_by_idx(self.idx, buf, lba).await?)
    }

    fn is_valid_header(&self, buf: &mut [u8]) -> bool {
        let header: &mut GPTHeader = bytemuck::from_bytes_mut(&mut buf[0..size_of::<GPTHeader>()]);

        let crc = header.header_crc32;
        header.header_crc32 = 0;

        let ok = crypto::crc32::is_verified_crc32(bytemuck::bytes_of(header), crc);
        log!("Header CRC validation result={}", ok);
        ok
    }

    async fn is_normal_present(&self) -> bool {
        log!("Checking primary GPT presence at LBA 1");
        let buf: Buffer = [0u32; 128].as_slice().into();
        if self.read_sectors_async(1, buf.clone()).await.is_err() {
            log!("Failed to read primary GPT sector");
            return false;
        }

        let present = buf.starts_with(b"EFI PART");
        if present {
            log!("Primary GPT signature found");
        } else {
            log!("Primary GPT signature not found");
        }

        present
    }

    async fn is_backup_present(&self) -> bool {
        log!("Checking backup GPT presence at LBA -1");
        let buf: Buffer = [0u32; 128].as_slice().into();
        if self.read_sectors_async(-1, buf.clone()).await.is_err() {
            log!("Failed to read backup GPT sector");
            return false;
        }

        let present = buf.starts_with(b"EFI PART");
        if present {
            log!("Backup GPT signature found");
        } else {
            log!("Backup GPT signature not found");
        }

        present
    }

    pub async fn is_gpt_present(&self) -> bool {
        log!("Checking if GPT is present (primary or backup)");
        let normal = self.is_normal_present().await;
        if normal {
            log!("GPT present on primary");
            return true;
        }

        let backup = self.is_backup_present().await;
        if backup {
            log!("GPT present on backup");
        } else {
            log!("No GPT detected on primary or backup");
        }

        normal || backup
    }

    pub async fn get_table(
        &self,
        lba: i64,
        is_backup: bool,
    ) -> Result<(GPTHeader, Vec<GPTEntry>), GPTErr> {
        log!("Reading GPT table at lba={} (is_backup={})", lba, is_backup);

        // Read header
        let handle = Self::get_buffer();
        let mut header_buf: Buffer = handle.get_buffer();
        self.read_sectors_async(lba, header_buf.clone())
            .await
            .map_err(|e| {
                log!(
                    "Failed to read GPT header at lba={}: {}",
                    lba,
                    e.to_string()
                );
                GPTErr::Io(e.to_string())
            })?;

        if !self.is_valid_header(&mut header_buf) {
            log!("Invalid GPT header detected at lba={}", lba);
            return Err(GPTErr::GPTCorrupted);
        }

        let result_header: GPTHeader =
            *bytemuck::from_bytes(&header_buf[0..size_of::<GPTHeader>()]);

        if !(result_header.entry_size / 128).is_power_of_two() {
            let entry_size = result_header.entry_size;
            log!("GPT entry size appears invalid: {}", entry_size);
            return Err(GPTErr::BadArrayEntrySize);
        }

        let arr_block_count: i64 = ((result_header.entry_num * result_header.entry_size / 512)
            + !(result_header.entry_num * result_header.entry_size).is_multiple_of(512) as u32)
            .into();

        let arr_lba: i64 = if is_backup {
            -1 - arr_block_count
        } else {
            result_header.array_start as i64
        };

        log!(
            "Reading GPT array at lba={} (blocks={})",
            arr_lba,
            arr_block_count
        );

        let arr_sectors = (result_header.entry_num * result_header.entry_size / 512) as u16;
        let arr_buf = vec![0u32; arr_sectors as usize * 512 / 4].into_boxed_slice();
        let buffer: Buffer = arr_buf.into();

        self.read_sectors_async(arr_lba, buffer.clone())
            .await
            .map_err(|e| {
                log!(
                    "Failed to read GPT array at lba={}: {}",
                    arr_lba,
                    e.to_string()
                );
                GPTErr::Io(e.to_string())
            })?;

        if !crypto::crc32::is_verified_crc32(buffer.deref(), result_header.array_crc32) {
            let crc = result_header.array_crc32;
            log!("GPT array CRC mismatch: expected={} (lba={})", crc, arr_lba);
            return Err(GPTErr::GPTCorrupted);
        }

        let result_array: Vec<GPTEntry> = buffer
            .deref()
            .chunks(result_header.entry_size as usize)
            .map(|slice| *bytemuck::from_bytes(&slice[0..size_of::<GPTEntry>()]))
            .collect();

        let entry_num = result_header.entry_num;
        log!(
            "Successfully read GPT header and array (entries={})",
            entry_num
        );
        Ok((result_header, result_array))
    }

    pub async fn read_gpt(&self) -> Result<(GPTHeader, Vec<GPTEntry>), GPTErr> {
        log!("Reading GPT (primary + backup)");
        if !self.is_gpt_present().await {
            log!("No GPT present when attempting to read");
            return Err(GPTErr::GPTNonExist);
        }

        let primary_result = self.get_table(1, false).await;
        let backup_result = self.get_table(-1, true).await;

        if let Ok((primary_header, primary_array)) = primary_result.as_ref()
            && let Ok((backup_header, backup_array)) = backup_result.as_ref()
        {
            if primary_header != backup_header || primary_array != backup_array {
                log!("Primary table differs from backup; synchronization needed");
                // TODO sync this
            }

            log!("Primary and backup GPT match (or acceptable)");
            Ok((*primary_header, primary_array.to_vec()))
        } else if let Ok((primary_header, primary_array)) = primary_result.as_ref()
            && let Err(e) = backup_result.as_ref()
        {
            log!("Primary ok but backup corrupted: {:?}", e);
            Ok((*primary_header, primary_array.to_vec()))
        } else if let Err(e) = primary_result
            && let Ok((secondary_header, secondary_array)) = backup_result
        {
            log!("Backup ok but primary corrupted: {:?}", e);
            Ok((secondary_header, secondary_array))
        } else {
            log!("Both primary and backup GPT are corrupted");
            Err(GPTErr::GPTCorrupted)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{end_test, ignore, test_name};

    #[test_case]
    #[allow(unreachable_code)]
    fn gptheader() {
        ignore!();
        test_name!("gpt header serialization");
        end_test!();
    }

    #[test_case]
    #[allow(unreachable_code)]
    fn gpt_present() {
        ignore!();
        test_name!("tests for is_gpt_present");
        end_test!();
    }
}
