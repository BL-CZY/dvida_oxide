use alloc::vec;
use alloc::vec::Vec;

use crate::utils::guid::Guid;

use super::storage::HalStorageContext;

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

impl HalStorageContext {
    pub fn is_gpt_present() -> bool {
        false
    }

    pub fn create_gpt() {}
}

#[cfg(test)]
mod tests {
    use crate::{ignore, test_name};

    #[test_case]
    #[allow(unreachable_code)]
    fn gptheader() {
        ignore!("gpt header serialization");
        test_name!("gpt header serialization");
    }
}
