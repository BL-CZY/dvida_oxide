use alloc::vec;
use alloc::vec::Vec;

use crate::utils;
use crate::utils::guid::Guid;

use super::storage::HalStorageDevice;

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
        assert_eq!(vec.len(), 92);

        vec
    }
}

impl HalStorageDevice {
    pub fn is_gpt_present(&mut self) -> bool {
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

    pub fn create_gpt() {}
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
