use core::error::Error;

use alloc::vec;
use alloc::{boxed::Box, vec::Vec};
use dvida_serialize::{DvDeserialize, DvSerialize, Endianness};
use terminal::log;

use crate::drivers::fs::ext2::INODES_PER_GROUP;
use crate::hal::storage::write_sectors;
use crate::{
    crypto::uuid::uuid_v4,
    drivers::fs::ext2::{
        ALGO_BITMAP, BLOCK_SIZE, BLOCKS_PER_GROUP, CREATOR_OS_DVIDA, DirEntry, EXT2_DYNAMIC_REV,
        EXT2_ERRORS_CONTINUE, EXT2_FEATURE_COMPAT_EXT_ATTR, EXT2_FEATURE_INCOMPAT_FILETYPE,
        EXT2_FEATURE_RO_COMPAT_SPARSE_SUPER, EXT2_FT_DIR, EXT2_OS_LINUX, EXT2_ROOT_INO,
        EXT2_S_IFDIR, EXT2_SUPER_MAGIC, EXT2_VALID_FS, FIRST_DATA_BLOCK, GroupDescriptor, Inode,
        LOG_BLOCK_SIZE, MAX_MOUNT_COUNT, ROOT_ID, S_R_BLOCKS_COUNT, SuperBlock,
    },
    hal::{
        self,
        gpt::GPTEntry,
        storage::{HalStorageOperationResult, read_sectors},
    },
    time::{self, Rtc, RtcDateTime, formats::rtc_to_posix},
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

// ext2 structure sizes (in bytes) according to spec
const SUPERBLOCK_SIZE: usize = 1024;
const GROUP_DESCRIPTOR_SIZE: usize = 32;
const INODE_SIZE: u16 = 128;
const DIR_ENTRY_HEADER_SIZE: usize = 8;
const FIRST_DATA_BLOCK_ADDR: u32 = 1024 / BLOCK_SIZE;
const SET_LATER: u32 = 0;

// blocksize is considered to be 1kb
pub async fn init_ext2(drive_id: usize, entry: &GPTEntry) -> Result<(), Box<dyn Error>> {
    // TODO: check entry eligibility

    let time = rtc_to_posix(&Rtc::new().read_datetime().expect("No time acquired"));

    let mut free_blocks_count: u32 = 0;
    let mut free_inodes_count: u32 = 0;

    let num_lba = entry.end_lba - entry.start_lba + 1;
    let num_block_groups = (((num_lba as i64 / (BLOCK_SIZE / 512) as i64)
        / BLOCKS_PER_GROUP as i64)
        + ((num_lba as i64 / (BLOCK_SIZE / 512) as i64) % BLOCKS_PER_GROUP as i64)
        != 0) as u32;

    let mut bg_vec: Vec<GroupDescriptor> = vec![];

    for i in 0..num_block_groups {
        // TODO: handle the case where the last block group is not big enough

        let offset = i * (2_u32.pow(LOG_BLOCK_SIZE)) * BLOCKS_PER_GROUP;

        let bg_descriptor = GroupDescriptor {
            bg_block_bitmap: offset + 2,
            bg_inode_bitmap: offset + 3,
            bg_inode_table: offset + 4,
            bg_free_blocks_count: (BLOCKS_PER_GROUP
                - ((INODE_SIZE as u32) * INODES_PER_GROUP / BLOCK_SIZE)
                - 5) as u16,
            bg_free_inodes_count: INODES_PER_GROUP as u16,
            bg_used_dirs_count: 0,
        };

        free_blocks_count += bg_descriptor.bg_free_blocks_count as u32;
        free_inodes_count += bg_descriptor.bg_free_inodes_count as u32;
        bg_vec.push(bg_descriptor);
    }

    let mut super_block: SuperBlock = SuperBlock {
        s_inodes_count: free_inodes_count,
        s_blocks_count: free_blocks_count,
        s_r_blocks_count: 0,
        s_free_blocks_count: free_blocks_count,
        s_free_inodes_count: free_inodes_count,
        s_first_data_block: FIRST_DATA_BLOCK_ADDR,
        s_log_block_size: LOG_BLOCK_SIZE,
        s_log_frag_size: LOG_BLOCK_SIZE, // this is not supported
        s_blocks_per_group: BLOCKS_PER_GROUP,
        s_frags_per_group: BLOCKS_PER_GROUP,
        s_inodes_per_group: BLOCKS_PER_GROUP,
        s_mtime: time,
        s_wtime: time,
        s_mnt_count: 1,
        s_max_mnt_count: MAX_MOUNT_COUNT,
        s_magic: EXT2_SUPER_MAGIC,
        s_state: EXT2_VALID_FS,
        s_errors: EXT2_ERRORS_CONTINUE,
        s_minor_rev_level: 0,
        s_lastcheck: time,
        s_checkinterval: rtc_to_posix(&RtcDateTime {
            year: 0,
            month: 6,
            weekday: 0,
            day: 0,
            hour: 0,
            minute: 0,
            second: 0,
        }),
        s_creator_os: CREATOR_OS_DVIDA,
        s_rev_level: 0,
        s_def_resuid: ROOT_ID,
        s_def_resgid: ROOT_ID,

        s_first_ino: 2 + (BLOCK_SIZE / 512) + 4,
        s_inode_size: INODE_SIZE,
        s_block_group_nr: bg_vec.len() as u16,

        s_feature_compat: 0,
        s_feature_incompat: 0,
        s_feature_ro_compat: 0,

        s_uuid: *uuid_v4().await.as_bytes(),
        s_volume_name: [0x41; 16],
        s_last_mounted: [0u8; 64],
        s_algo_bitmap: ALGO_BITMAP,

        s_prealloc_blocks: 1,
        s_prealloc_dir_blocks: 1,
        s_padding1: 0,

        // TODO: pretty much everything below
        s_journal_uuid: [0; 16],
        s_journal_inum: 0,
        s_journal_dev: 0,
        s_last_orphan: 0,

        s_hash_seed: [0; 4],
        s_def_hash_version: 0,
        reserved: [0u8; 3],

        s_default_mount_opts: 0,
        s_first_meta_bg: 0,
    };

    let mut buffer = [0u8; 1024];
    super_block.serialize(Endianness::Little, &mut buffer)?;

    write_sectors(drive_id, buffer, entry.start_lba + 2).await?;

    Ok(())
}
