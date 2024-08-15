use crate::println;
use alloc::vec;
use alloc::vec::Vec;

use crate::utils;
use crate::utils::guid::Guid;

use super::storage::{HalStorageDevice, IoErr};

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

impl GPTHeader {
    pub fn from_buf(buf: &Vec<u8>) -> Self {
        GPTHeader {
            sig: buf[0..8].try_into().unwrap(),
            revision: u32::from_le_bytes(buf[8..12].try_into().unwrap()),
            size: u32::from_le_bytes(buf[12..16].try_into().unwrap()),
            header_crc32: u32::from_le_bytes(buf[16..20].try_into().unwrap()),
            reserved: 0,
            loc: u64::from_le_bytes(buf[24..32].try_into().unwrap()),
            backup_loc: u64::from_le_bytes(buf[32..40].try_into().unwrap()),
            first_usable_block: u64::from_le_bytes(buf[40..48].try_into().unwrap()),
            last_usable_block: u64::from_le_bytes(buf[48..56].try_into().unwrap()),
            guid: Guid::from_buf(buf[56..72].try_into().unwrap()),
            array_start: u64::from_le_bytes(buf[72..80].try_into().unwrap()),
            entry_num: u32::from_le_bytes(buf[80..84].try_into().unwrap()),
            entry_size: u32::from_le_bytes(buf[84..88].try_into().unwrap()),
            array_crc32: u32::from_le_bytes(buf[88..92].try_into().unwrap()),
        }
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
}

#[derive(PartialEq, Eq, Clone)]
pub struct GPTEntry {
    type_guid: Guid,
    unique_guid: Guid,
    start_lba: u64,
    end_lba: u64,
    flags: u64,
    name: [u16; 36],
}

impl GPTEntry {
    pub fn try_from_buf(buf: &[u8]) -> Result<Self, ()> {
        if !(buf.len() / 128).is_power_of_two() {
            return Err(());
        }

        Ok(GPTEntry {
            type_guid: Guid::from_buf(buf[0..16].try_into().unwrap()),
            unique_guid: Guid::from_buf(buf[16..32].try_into().unwrap()),
            start_lba: u64::from_le_bytes(buf[32..40].try_into().unwrap()),
            end_lba: u64::from_le_bytes(buf[40..48].try_into().unwrap()),
            flags: u64::from_le_bytes(buf[48..56].try_into().unwrap()),
            name: buf[56..]
                .windows(2)
                .map(|slice| u16::from_le_bytes(slice.try_into().unwrap()))
                .collect::<Vec<u16>>()
                .try_into()
                .unwrap(),
        })
    }

    pub fn to_buf(&self) -> Vec<u8> {
        let mut result = vec![];

        result.extend(self.type_guid.to_buf());
        result.extend(self.unique_guid.to_buf());
        result.extend(self.start_lba.to_le_bytes());
        result.extend(self.end_lba.to_le_bytes());
        result.extend(self.flags.to_le_bytes());
        self.name
            .iter()
            .inspect(|character| result.extend(character.to_le_bytes()));

        result
    }
}

#[derive(Debug)]
pub enum GPTWriteErr {
    GPTAlreadyExist,
    ErrWritingBuf(IoErr),
}

#[derive(Debug)]
pub enum GPTReadErr {
    GPTNonExist,
    GPTCorrupted,
    BadArrayEntrySize,
    ErrReadingBuf(IoErr),
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
            utils::lba_to_chs(self.sectors_per_track(), self.highest_lba());

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

        if self.highest_lba() > 0xFFFFFFFF {
            result[PMBR_OFFSET + 12] = 0xFF;
            result[PMBR_OFFSET + 13] = 0xFF;
            result[PMBR_OFFSET + 14] = 0xFF;
            result[PMBR_OFFSET + 15] = 0xFF;
        } else {
            let temp = self.highest_lba() as u32;
            result[PMBR_OFFSET + 12] = temp.to_le_bytes()[0];
            result[PMBR_OFFSET + 13] = temp.to_le_bytes()[1];
            result[PMBR_OFFSET + 14] = temp.to_le_bytes()[2];
            result[PMBR_OFFSET + 15] = temp.to_le_bytes()[3];
        }

        result[510] = 0x55;
        result[511] = 0xAA;

        result
    }

    fn create_unhashed_header_buf(&self) -> Vec<u8> {
        let mut result: Vec<u8> = vec![];

        result.extend(b"EFI PART");
        result.extend(1u32.to_le_bytes());
        result.extend(92u32.to_le_bytes());
        result.extend([0u8; 8]);
        result.extend(1u32.to_le_bytes());
        result.extend((self.highest_lba() - 1).to_le_bytes());
        result.extend(34u32.to_le_bytes());
        result.extend((self.highest_lba() - 34).to_le_bytes());
        result.extend(Guid::new().to_buf());
        result.extend(2u32.to_le_bytes());
        result.extend(128u32.to_le_bytes());
        result.extend(128u32.to_le_bytes());

        result
    }

    fn hash_header_buf(&self, buf: &mut Vec<u8>, array_crc32: u32) {
        buf.extend(array_crc32.to_le_bytes());
        let crc32 = utils::crc32::full_crc(&buf);
        for (index, val) in crc32.to_le_bytes().iter().enumerate() {
            buf[16 + index] = *val;
        }
    }

    fn write_pmbr(&mut self, pmbr: &Vec<u8>) -> Result<(), IoErr> {
        if let Err(e) = self.write_sectors(0, 1, pmbr) {
            return Err(e);
        };

        Ok(())
    }

    fn write_table(&mut self, header: &Vec<u8>, array: &Vec<u8>) -> Result<(), IoErr> {
        if let Err(e) = self.write_sectors(1, 1, header) {
            return Err(e);
        }

        if let Err(e) = self.write_sectors(2, 32, array) {
            return Err(e);
        }

        if let Err(e) = self.write_sectors(-1, 1, header) {
            return Err(e);
        }

        if let Err(e) = self.write_sectors(-33, 32, array) {
            return Err(e);
        };

        Ok(())
    }

    pub fn create_gpt(&mut self, force: bool) -> Result<(), GPTWriteErr> {
        if !force && self.is_gpt_present() {
            return Err(GPTWriteErr::GPTAlreadyExist);
        }

        let pmbr: Vec<u8> = self.create_pmbr_buf();
        let mut header: Vec<u8> = self.create_unhashed_header_buf();
        let array: Vec<u8> = Vec::from([0; 32 * 512]);
        let array_crc32 = utils::crc32::full_crc(&array);
        self.hash_header_buf(&mut header, array_crc32);

        if let Err(e) = self.write_pmbr(&pmbr) {
            return Err(GPTWriteErr::ErrWritingBuf(e));
        }

        if let Err(e) = self.write_table(&header, &array) {
            return Err(GPTWriteErr::ErrWritingBuf(e));
        }

        Ok(())
    }

    fn is_valid_header(&self, buf: &mut Vec<u8>) -> bool {
        let crc32 = u32::from_be_bytes(buf[16..20].try_into().unwrap());
        buf[16] = 0;
        buf[17] = 0;
        buf[18] = 0;
        buf[19] = 0;
        utils::crc32::is_verified_crc32(buf, crc32)
    }

    pub fn check_table(
        &mut self,
        lba: i64,
        is_backup: bool,
    ) -> Result<(GPTHeader, Vec<GPTEntry>), GPTReadErr> {
        // read buffer
        let mut header_buf = match self.read_sectors(lba, 1) {
            Ok(res) => res,
            Err(e) => return Err(GPTReadErr::ErrReadingBuf(e)),
        };

        if !self.is_valid_header(&mut header_buf) {
            return Err(GPTReadErr::GPTCorrupted);
        }

        let result_header = GPTHeader::from_buf(&header_buf);

        if !(result_header.entry_size / 128).is_power_of_two() {
            return Err(GPTReadErr::BadArrayEntrySize);
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

        let arr_buf = match self.read_sectors(
            arr_lba,
            (result_header.entry_num * result_header.entry_size / 512) as u16,
        ) {
            Ok(res) => res,
            Err(e) => return Err(GPTReadErr::ErrReadingBuf(e)),
        };

        if !utils::crc32::is_verified_crc32(&arr_buf, result_header.array_crc32) {
            return Err(GPTReadErr::GPTCorrupted);
        }

        let result_array: Vec<GPTEntry> = arr_buf
            .windows(result_header.entry_size as usize)
            // unwrap because we are sure that this function will not throw an error
            // entry size is a 128 * 2^n
            .map(|slice| GPTEntry::try_from_buf(slice).unwrap())
            .collect::<Vec<GPTEntry>>();

        Ok((result_header, result_array))
    }

    pub fn read_gpt(&mut self) -> Result<(GPTHeader, Vec<GPTEntry>), GPTReadErr> {
        if !self.is_gpt_present() {
            return Err(GPTReadErr::GPTNonExist);
        }

        let primary_result = self.check_table(1, false);
        let backup_result = self.check_table(-1, false);

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
            return Err(GPTReadErr::GPTCorrupted);
        }
    }

    pub fn add_entry(&mut self) {}

    pub fn delete_entry(&mut self, index: u32) {}
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
