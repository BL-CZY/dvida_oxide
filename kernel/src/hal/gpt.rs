use super::storage::HalStorageContext;

pub struct GPTHeader {
    sig: [u8; 8],
    revision: [u8; 4],
    size: u32,
    header_crc32: u32,
    reserved: u32,
    lba: u64,
    backup_lba: u64,
    first_usable_block: u64,
    last_usable_block: u64,
}

impl HalStorageContext {
    pub fn is_gpt_present() -> bool {
        false
    }

    pub fn create_gpt() {}
}
