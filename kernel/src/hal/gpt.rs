use crate::println;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::{boxed::Box, vec};
use thiserror::Error;

use crate::utils;
use crate::utils::guid::Guid;

use super::storage::HalStorageDevice;

#[derive(PartialEq, Eq, Clone, Copy)]
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
    guid: Guid,
    array_start: u64,
    entry_num: u32,
    entry_size: u32,
    array_crc32: u32,
}

impl Default for GPTHeader {
    fn default() -> Self {
        GPTHeader {
            sig: *b"EFI PART",
            revision: 0x00010000,
            size: 0x5C,
            header_crc32: 0,
            reserved: 0,
            loc: 1,
            backup_loc: 0,
            first_usable_block: 0x22,
            last_usable_block: 0,
            guid: Guid::new(),
            array_start: 2,
            entry_num: 0x80,
            entry_size: 0x80,
            array_crc32: 0,
        }
    }
}

impl Into<Vec<u8>> for GPTHeader {
    fn into(self) -> Vec<u8> {
        self.to_buf()
    }
}

impl TryFrom<&[u8]> for GPTHeader {
    type Error = Box<dyn core::error::Error>;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() > 92 {
            return Err(Box::new(GPTErr::BufferTooSmall));
        }

        Ok(GPTHeader {
            sig: value[0..8].try_into().unwrap(),
            revision: u32::from_le_bytes(value[8..12].try_into().unwrap()),
            size: u32::from_le_bytes(value[12..16].try_into().unwrap()),
            header_crc32: u32::from_le_bytes(value[16..20].try_into().unwrap()),
            reserved: 0,
            loc: u64::from_le_bytes(value[24..32].try_into().unwrap()),
            backup_loc: u64::from_le_bytes(value[32..40].try_into().unwrap()),
            first_usable_block: u64::from_le_bytes(value[40..48].try_into().unwrap()),
            last_usable_block: u64::from_le_bytes(value[48..56].try_into().unwrap()),
            guid: Guid::from_buf(value[56..72].try_into().unwrap()),
            array_start: u64::from_le_bytes(value[72..80].try_into().unwrap()),
            entry_num: u32::from_le_bytes(value[80..84].try_into().unwrap()),
            entry_size: u32::from_le_bytes(value[84..88].try_into().unwrap()),
            array_crc32: u32::from_le_bytes(value[88..92].try_into().unwrap()),
        })
    }
}

impl GPTHeader {
    pub fn try_from_buf(buf: &[u8]) -> Result<Self, Box<dyn core::error::Error>> {
        GPTHeader::try_from(buf)
    }

    pub fn to_buf(&self) -> Vec<u8> {
        let mut vec: Vec<u8> = vec![];
        vec.extend(&self.sig);
        vec.extend(&self.revision.to_le_bytes());
        vec.extend(&self.size.to_le_bytes());
        vec.extend(&self.header_crc32.to_le_bytes());
        vec.extend(&self.reserved.to_le_bytes());
        vec.extend(&self.loc.to_le_bytes());
        vec.extend(&self.backup_loc.to_le_bytes());
        vec.extend(&self.first_usable_block.to_le_bytes());
        vec.extend(&self.last_usable_block.to_le_bytes());
        vec.extend(&self.guid.to_buf());
        vec.extend(&self.array_start.to_le_bytes());
        vec.extend(&self.entry_num.to_le_bytes());
        vec.extend(&self.entry_size.to_le_bytes());
        vec.extend(&self.array_crc32.to_le_bytes());

        vec
    }

    pub fn to_buf_full(&self) -> Vec<u8> {
        let mut result = self.to_buf();
        while result.len() < 512 {
            result.push(0);
        }

        result
    }
}

#[derive(PartialEq, Eq, Clone)]
pub struct GPTEntry {
    type_guid: Guid,
    unique_guid: Guid,
    start_lba: u64,
    end_lba: u64,
    flags: u64,
    name: String,
}

impl TryFrom<&[u8]> for GPTEntry {
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if (value.len() / 128) % 2 != 0 {
            return Err(Box::new(GPTErr::BadArrayEntrySize));
        }

        Ok(GPTEntry {
            type_guid: Guid::from_buf(value[0..16].try_into().unwrap()),
            unique_guid: Guid::from_buf(value[16..32].try_into().unwrap()),
            start_lba: u64::from_le_bytes(value[32..40].try_into().unwrap()),
            end_lba: u64::from_le_bytes(value[40..48].try_into().unwrap()),
            flags: u64::from_le_bytes(value[48..56].try_into().unwrap()),
            name: String::from_utf16(
                value[56..]
                    .windows(2)
                    .map(|slice| u16::from_le_bytes(slice.try_into().unwrap()))
                    .collect::<Vec<u16>>()
                    .as_slice(),
            )?,
        })
    }

    type Error = Box<dyn core::error::Error>;
}

impl Into<Vec<u8>> for GPTEntry {
    fn into(self) -> Vec<u8> {
        self.to_buf()
    }
}

impl GPTEntry {
    pub fn try_from_buf(buf: &[u8]) -> Result<Self, Box<dyn core::error::Error>> {
        buf.try_into()
    }

    pub fn to_buf(&self) -> Vec<u8> {
        let mut result = vec![];

        result.extend(self.type_guid.to_buf());
        result.extend(self.unique_guid.to_buf());
        result.extend(self.start_lba.to_le_bytes());
        result.extend(self.end_lba.to_le_bytes());
        result.extend(self.flags.to_le_bytes());
        result.extend(
            self.name
                .encode_utf16()
                .map(|ele| ele.to_le_bytes())
                .flatten()
                .collect::<Vec<u8>>(),
        );

        result
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
}

impl HalStorageDevice {
    fn is_normal_present(&mut self) -> bool {
        let buf: Vec<u8> = if let Ok(res) = self.read_sectors(1, 1) {
            res
        } else {
            return false;
        };

        if buf.starts_with(b"EFI PART") {
            true
        } else {
            false
        }
    }

    fn is_backup_present(&mut self) -> bool {
        let buf: Vec<u8> = if let Ok(res) = self.read_sectors(-1, 1) {
            res
        } else {
            return false;
        };

        if buf.starts_with(b"EFI PART") {
            true
        } else {
            false
        }
    }

    pub fn is_gpt_present(&mut self) -> bool {
        self.is_normal_present() || self.is_backup_present()
    }

    fn create_pmbr_buf(&self) -> Vec<u8> {
        const PMBR_OFFSET: usize = 446;
        let mut result = Vec::from([0; 512]);

        result[PMBR_OFFSET + 1] = 0x0;
        result[PMBR_OFFSET + 2] = 0x2;
        result[PMBR_OFFSET + 3] = 0x0;
        result[PMBR_OFFSET + 4] = 0xEE;

        let (cylinder, head, sector) =
            utils::lba_to_chs(self.sectors_per_track(), self.sector_count());

        if cylinder > 0xFF || head > 0xFF || sector > 0xFF {
            result[PMBR_OFFSET + 5] = 0xFF;
            result[PMBR_OFFSET + 6] = 0xFF;
            result[PMBR_OFFSET + 7] = 0xFF;
        } else {
            result[PMBR_OFFSET + 5] = cylinder as u8;
            result[PMBR_OFFSET + 6] = head as u8;
            result[PMBR_OFFSET + 7] = sector as u8;
        }

        result[PMBR_OFFSET + 8] = 0x1;
        result[PMBR_OFFSET + 9] = 0x0;
        result[PMBR_OFFSET + 10] = 0x0;
        result[PMBR_OFFSET + 11] = 0x0;

        if self.sector_count() > 0xFFFFFFFF {
            result[PMBR_OFFSET + 12] = 0xFF;
            result[PMBR_OFFSET + 13] = 0xFF;
            result[PMBR_OFFSET + 14] = 0xFF;
            result[PMBR_OFFSET + 15] = 0xFF;
        } else {
            let temp = self.sector_count() as u32;
            result[PMBR_OFFSET + 12] = temp.to_le_bytes()[0];
            result[PMBR_OFFSET + 13] = temp.to_le_bytes()[1];
            result[PMBR_OFFSET + 14] = temp.to_le_bytes()[2];
            result[PMBR_OFFSET + 15] = temp.to_le_bytes()[3];
        }

        result[510] = 0x55;
        result[511] = 0xAA;

        result
    }

    fn create_unhashed_header(&self) -> GPTHeader {
        GPTHeader {
            backup_loc: self.sector_count() - 1,
            last_usable_block: self.sector_count() - 34,
            ..Default::default()
        }
    }

    fn write_pmbr(&mut self, pmbr: &Vec<u8>) -> Result<(), Box<dyn core::error::Error>> {
        self.write_sectors(0, 1, pmbr)?;

        Ok(())
    }

    fn write_table(
        &mut self,
        header: &[u8],
        array: &[u8],
    ) -> Result<(), Box<dyn core::error::Error>> {
        self.write_sectors(1, 1, header)?;

        self.write_sectors(2, 32, array)?;

        self.write_sectors(-1, 1, header)?;

        self.write_sectors(-33, 32, array)?;

        Ok(())
    }

    pub fn create_gpt(&mut self, force: bool) -> Result<(), Box<dyn core::error::Error>> {
        if !force && self.is_gpt_present() {
            return Err(Box::new(GPTErr::GPTAlreadyExist));
        }

        let pmbr: Vec<u8> = self.create_pmbr_buf();
        let mut header = self.create_unhashed_header();
        let array: Vec<u8> = Vec::from([0; 32 * 512]);
        header.array_crc32 = utils::crc32::full_crc(&array);
        header.header_crc32 = utils::crc32::full_crc(&header.to_buf());

        self.write_pmbr(&pmbr)?;
        self.write_table(&header.to_buf_full(), &array)?;

        Ok(())
    }

    fn is_valid_header(&self, buf: &[u8]) -> bool {
        let mut header: GPTHeader = if let Ok(h) = buf.try_into() {
            h
        } else {
            return false;
        };

        let crc = header.header_crc32;
        header.header_crc32 = 0;

        utils::crc32::is_verified_crc32(&header.to_buf(), crc)
    }

    pub fn get_table(
        &mut self,
        lba: i64,
        is_backup: bool,
    ) -> Result<(GPTHeader, Vec<GPTEntry>), Box<dyn core::error::Error>> {
        // read buffer
        let mut header_buf = self.read_sectors(lba, 1)?;

        if !self.is_valid_header(&mut header_buf) {
            return Err(Box::new(GPTErr::GPTCorrupted));
        }

        let result_header = GPTHeader::try_from(header_buf.as_slice())?;

        if (result_header.entry_size / 128) % 2 != 0 {
            return Err(Box::new(GPTErr::BadArrayEntrySize));
        }

        let arr_block_count: i64 = ((result_header.entry_num * result_header.entry_size / 512)
            + ((result_header.entry_num * result_header.entry_size) % 512)
            == 0)
            .into();

        let arr_lba: i64 = if is_backup {
            -1 - arr_block_count
        } else {
            result_header.array_start as i64
        };

        let arr_buf = self.read_sectors(
            arr_lba,
            (result_header.entry_num * result_header.entry_size / 512) as u16,
        )?;

        if !utils::crc32::is_verified_crc32(&arr_buf, result_header.array_crc32) {
            return Err(Box::new(GPTErr::GPTCorrupted));
        }

        let result_array: Vec<GPTEntry> = arr_buf
            .windows(result_header.entry_size as usize)
            // unwrap because we are sure that this function will not throw an error
            // entry size is a 128 * 2^n
            .map(|slice| GPTEntry::try_from_buf(slice).unwrap())
            .collect::<Vec<GPTEntry>>();

        Ok((result_header, result_array))
    }

    pub fn read_gpt(&mut self) -> Result<(GPTHeader, Vec<GPTEntry>), Box<dyn core::error::Error>> {
        if !self.is_gpt_present() {
            return Err(Box::new(GPTErr::GPTNonExist));
        }

        let primary_result = self.get_table(1, false);
        let backup_result = self.get_table(-1, false);

        if let Ok((primary_header, primary_array)) = primary_result.as_ref()
            && let Ok((backup_header, backup_array)) = backup_result.as_ref()
        {
            if primary_header != backup_header || primary_array != backup_array {
                println!(
                    "Primary table appears is different from the backup table, sync backup..."
                );
                // TODO sync this
            }

            return Ok((*primary_header, primary_array.to_vec()));
        } else if let Ok((primary_header, primary_array)) = primary_result.as_ref()
            && let Err(e) = backup_result.as_ref()
        {
            // TODO fix this
            println!(
                "Primary table appears ok, but the backup one is corrupted: {:?}",
                e
            );
            return Ok((*primary_header, primary_array.to_vec()));
        } else if let Err(e) = primary_result
            && let Ok((secondary_header, secondary_array)) = backup_result
        {
            // TODO fix this
            println!(
                "Backup table appears ok, but the primary one is corrupted: {:?}",
                e
            );

            return Ok((secondary_header, secondary_array));
        } else {
            return Err(Box::new(GPTErr::GPTCorrupted));
        }
    }

    pub fn add_entry(&mut self) {}

    pub fn delete_entry(&mut self, _index: u32) {}
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
