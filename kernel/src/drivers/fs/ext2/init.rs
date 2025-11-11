use alloc::boxed::Box;
use dvida_serialize::DvDeserialize;
use terminal::log;

use crate::{
    drivers::fs::ext2::{FIRST_DATA_BLOCK, LOG_BLOCK_SIZE, S_R_BLOCKS_COUNT, SuperBlock},
    hal::{
        gpt::GPTEntry,
        storage::{HalStorageOperationResult, read_sectors},
    },
};

pub async fn identify_ext2(drive_id: usize, entry: &GPTEntry) -> bool {
    let mut buf = Box::new([0u8; 1024]);

    if entry.start_lba - entry.end_lba < 3 {
        log!("Failed to identify ext2 because the GPT entry is too small");
        return false;
    }

    match read_sectors(drive_id, buf.clone(), (entry.start_lba + 1) as i64).await {
        HalStorageOperationResult::Success => {}
        HalStorageOperationResult::Failure(err) => {
            log!("Failed to identify ext2 because of read error: {}", err);
            return false;
        }
    }

    let super_block =
        match SuperBlock::deserialize(dvida_serialize::Endianness::Little, buf.as_mut_slice()) {
            Ok(res) => res.0,
            Err(e) => {
                log!(
                    "Failed to identify ext2 because of deserialization error: {:?}",
                    e
                );
                return false;
            }
        };

    if super_block.s_magic == 0xEF53 {
        true
    } else {
        false
    }
}

fn calculate_inodes_count(size: u64) -> u32 {
    0
}

fn calculate_blocks_count(size: u64) -> u32 {
    0
}

// pub async fn init_ext2(drive_id: usize, entry: &GPTEntry) {
//     let inodes_count = calculate_inodes_count(entry.end_lba - entry.start_lba + 1);
//     let blocks_count = calculate_blocks_count(entry.end_lba - entry.start_lba + 1);
//
//     let super_block = SuperBlock {
//         s_inodes_count: inodes_count,
//         s_blocks_count: blocks_count,
//         s_r_blocks_count: S_R_BLOCKS_COUNT,
//         s_free_blocks_count: blocks_count,
//         s_free_inodes_count: inodes_count,
//         s_first_data_block: FIRST_DATA_BLOCK,
//         s_log_block_size: LOG_BLOCK_SIZE,
//     };
// }
