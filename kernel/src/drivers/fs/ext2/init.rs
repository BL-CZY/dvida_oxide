use alloc::boxed::Box;
use alloc::vec;
use core::mem::size_of;
use dvida_serialize::DvDeserialize;
use terminal::log;

use crate::{
    drivers::fs::ext2::{
        DirEntry, EXT2_DYNAMIC_REV, EXT2_ERRORS_CONTINUE, EXT2_FEATURE_COMPAT_EXT_ATTR,
        EXT2_FEATURE_INCOMPAT_FILETYPE, EXT2_FEATURE_RO_COMPAT_SPARSE_SUPER, EXT2_FT_DIR,
        EXT2_OS_LINUX, EXT2_ROOT_INO, EXT2_S_IFDIR, EXT2_SUPER_MAGIC, EXT2_VALID_FS,
        FIRST_DATA_BLOCK, GroupDescriptor, Inode, LOG_BLOCK_SIZE, S_R_BLOCKS_COUNT, SuperBlock,
    },
    hal::{
        self,
        gpt::GPTEntry,
        storage::{HalStorageOperationResult, read_sectors},
    },
    time::{self, formats::rtc_to_posix},
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

fn calculate_inodes_count(size_in_blocks: u64) -> u32 {
    // Standard ratio: one inode per 16KB (16 blocks in ext2 with 1KB blocks)
    // Minimum 1 inode per 16 blocks, maximum based on blocks
    let inodes = size_in_blocks / 16;
    inodes.min(u32::MAX as u64).max(11) as u32 // At least 11 for reserved inodes
}

fn calculate_blocks_count(size_in_lba_blocks: u64) -> u32 {
    // GPT LBA blocks are 512 bytes, ext2 blocks are 1024 bytes
    // So we have half as many ext2 blocks as LBA blocks
    let blocks = size_in_lba_blocks / 2;
    blocks.min(u32::MAX as u64) as u32
}

pub async fn init_ext2(drive_id: usize, entry: &GPTEntry) {
    let partition_size_lba = entry.end_lba - entry.start_lba + 1;
    let inodes_count = calculate_inodes_count(partition_size_lba);
    let blocks_count = calculate_blocks_count(partition_size_lba);

    // Calculate filesystem geometry
    let blocks_per_group: u32 = 8192; // Standard: 8192 blocks per group
    let inodes_per_group: u32 =
        inodes_count / ((blocks_count + blocks_per_group - 1) / blocks_per_group).max(1);
    let block_groups_count = (blocks_count + blocks_per_group - 1) / blocks_per_group;

    // Get current time
    let current_time = time::Rtc::new()
        .read_datetime()
        .map(|dt| rtc_to_posix(&dt))
        .unwrap_or(0);

    // Create superblock
    let super_block = SuperBlock {
        s_inodes_count: inodes_count,
        s_blocks_count: blocks_count,
        s_r_blocks_count: S_R_BLOCKS_COUNT,
        s_free_blocks_count: blocks_count - block_groups_count * 5 - 1, // Reserve for metadata
        s_free_inodes_count: inodes_count - 10,                         // Reserve first 10 inodes
        s_first_data_block: FIRST_DATA_BLOCK,
        s_log_block_size: LOG_BLOCK_SIZE,
        s_log_frag_size: LOG_BLOCK_SIZE,
        s_blocks_per_group: blocks_per_group,
        s_frags_per_group: blocks_per_group,
        s_inodes_per_group: inodes_per_group,
        s_mtime: current_time,
        s_wtime: current_time,
        s_mnt_count: 0,
        s_max_mnt_count: 20,
        s_magic: EXT2_SUPER_MAGIC,
        s_state: EXT2_VALID_FS,
        s_errors: EXT2_ERRORS_CONTINUE,
        s_minor_rev_level: 0,
        s_lastcheck: current_time,
        s_checkinterval: 15552000, // 180 days in seconds
        s_creator_os: EXT2_OS_LINUX,
        s_rev_level: EXT2_DYNAMIC_REV,
        s_def_resuid: 0,
        s_def_resgid: 0,
        s_first_ino: 11,
        s_inode_size: 128,
        s_block_group_nr: 0,
        s_feature_compat: EXT2_FEATURE_COMPAT_EXT_ATTR,
        s_feature_incompat: EXT2_FEATURE_INCOMPAT_FILETYPE,
        s_feature_ro_compat: EXT2_FEATURE_RO_COMPAT_SPARSE_SUPER,
        s_uuid: [0u8; 16], // Should generate random UUID
        s_volume_name: [0u8; 16],
        s_last_mounted: [0u8; 64],
        s_algo_bitmap: 0,
        s_prealloc_blocks: 0,
        s_prealloc_dir_blocks: 0,
        s_padding1: 0,
        s_journal_uuid: [0u8; 16],
        s_journal_inum: 0,
        s_journal_dev: 0,
        s_last_orphan: 0,
        s_hash_seed: [0u32; 4],
        s_def_hash_version: 0,
        reserved: [0u8; 3],
        s_default_mount_opts: 0,
        s_first_meta_bg: 0,
    };

    // Write superblock at offset 1024 (2 LBA sectors)
    let mut sb_buffer = vec![0u8; 1024].into_boxed_slice();
    unsafe {
        let sb_ptr = &super_block as *const SuperBlock as *const u8;
        core::ptr::copy_nonoverlapping(sb_ptr, sb_buffer.as_mut_ptr(), size_of::<SuperBlock>());
    }
    let _ = hal::storage::write_sectors(drive_id, sb_buffer, entry.start_lba as i64 + 2i64).await;

    // Initialize block group descriptors
    let gdt_blocks = ((block_groups_count * size_of::<GroupDescriptor>() as u32) + 1023) / 1024;
    let mut gdt_buffer = vec![0u8; (gdt_blocks * 1024) as usize].into_boxed_slice();

    for bg in 0..block_groups_count {
        let gd = GroupDescriptor {
            bg_block_bitmap: FIRST_DATA_BLOCK + 1 + gdt_blocks + bg * blocks_per_group,
            bg_inode_bitmap: FIRST_DATA_BLOCK + 2 + gdt_blocks + bg * blocks_per_group,
            bg_inode_table: FIRST_DATA_BLOCK + 3 + gdt_blocks + bg * blocks_per_group,
            bg_free_blocks_count: if bg == 0 {
                (blocks_per_group - 5 - gdt_blocks - (inodes_per_group * 128 + 1023) / 1024) as u16
            } else {
                (blocks_per_group - 3 - (inodes_per_group * 128 + 1023) / 1024) as u16
            },
            bg_free_inodes_count: if bg == 0 {
                (inodes_per_group - 10) as u16
            } else {
                inodes_per_group as u16
            },
            bg_used_dirs_count: if bg == 0 { 1 } else { 0 },
            bg_flags: 0,
            bg_exclude_bitmap_lo: 0,
            bg_block_bitmap_csum_lo: 0,
            bg_inode_bitmap_csum_lo: 0,
            bg_itable_unused: 0,
            bg_checksum: 0,
        };

        unsafe {
            let gd_ptr = &gd as *const GroupDescriptor as *const u8;
            let offset = (bg * size_of::<GroupDescriptor>() as u32) as usize;
            core::ptr::copy_nonoverlapping(
                gd_ptr,
                gdt_buffer.as_mut_ptr().add(offset),
                size_of::<GroupDescriptor>(),
            );
        }
    }

    // Write GDT after superblock
    let _ = hal::storage::write_sectors(drive_id, gdt_buffer, entry.start_lba as i64 + 4).await;

    // Initialize root directory inode (inode 2)
    let root_inode = Inode {
        i_mode: EXT2_S_IFDIR | 0o755,
        i_uid: 0,
        i_size: 1024,
        i_atime: current_time,
        i_ctime: current_time,
        i_mtime: current_time,
        i_dtime: 0,
        i_gid: 0,
        i_links_count: 2, // . and ..
        i_blocks: 2,      // 2 * 512-byte blocks = 1024 bytes
        i_flags: 0,
        i_osd1: 0,
        i_block: {
            let mut blocks = [0u32; 15];
            blocks[0] = FIRST_DATA_BLOCK + 4 + gdt_blocks; // First data block for root
            blocks
        },
        i_generation: 0,
        i_file_acl: 0,
        i_dir_acl: 0,
        i_faddr: 0,
        i_osd2: [0u8; 12],
    };

    // Write root inode to inode table
    let inode_table_lba = entry.start_lba as i64 + ((FIRST_DATA_BLOCK + 3 + gdt_blocks) * 2) as i64;
    let mut inode_buffer = vec![0u8; 1024].into_boxed_slice();
    unsafe {
        let inode_ptr = &root_inode as *const Inode as *const u8;
        core::ptr::copy_nonoverlapping(
            inode_ptr,
            inode_buffer.as_mut_ptr().add(128),
            size_of::<Inode>(),
        );
    }
    let _ = hal::storage::write_sectors(drive_id, inode_buffer, inode_table_lba).await;

    // Create root directory entries
    let mut root_dir_buffer = vec![0u8; 1024].into_boxed_slice();

    // Entry for "." (current directory)
    let dot_entry = DirEntry {
        inode: EXT2_ROOT_INO,
        rec_len: 12,
        name_len: 1,
        file_type: EXT2_FT_DIR,
    };

    // Entry for ".." (parent directory, also root)
    let dotdot_entry = DirEntry {
        inode: EXT2_ROOT_INO,
        rec_len: 1012, // Rest of block
        name_len: 2,
        file_type: EXT2_FT_DIR,
    };

    unsafe {
        // Write "." entry
        let dot_ptr = &dot_entry as *const DirEntry as *const u8;
        core::ptr::copy_nonoverlapping(
            dot_ptr,
            root_dir_buffer.as_mut_ptr(),
            size_of::<DirEntry>(),
        );
        root_dir_buffer[size_of::<DirEntry>()] = b'.';

        // Write ".." entry
        let dotdot_ptr = &dotdot_entry as *const DirEntry as *const u8;
        core::ptr::copy_nonoverlapping(
            dotdot_ptr,
            root_dir_buffer.as_mut_ptr().add(12),
            size_of::<DirEntry>(),
        );
        root_dir_buffer[12 + size_of::<DirEntry>()] = b'.';
        root_dir_buffer[12 + size_of::<DirEntry>() + 1] = b'.';
    }

    // Write root directory data
    let root_data_lba = entry.start_lba as i64 + ((FIRST_DATA_BLOCK + 4 + gdt_blocks) * 2) as i64;
    let _ = hal::storage::write_sectors(drive_id, root_dir_buffer, root_data_lba).await;

    // Initialize block bitmap (mark used blocks)
    let mut block_bitmap = vec![0u8; 1024].into_boxed_slice();
    // Mark first few blocks as used (superblock, GDT, bitmaps, inode table, root data)
    let used_blocks = 5 + gdt_blocks + (inodes_per_group * 128 + 1023) / 1024;
    for i in 0..used_blocks {
        let byte_idx = (i / 8) as usize;
        let bit_idx = (i % 8) as u8;
        block_bitmap[byte_idx] |= 1 << bit_idx;
    }
    let block_bitmap_lba =
        entry.start_lba as i64 + ((FIRST_DATA_BLOCK + 1 + gdt_blocks) * 2) as i64;
    let _ = hal::storage::write_sectors(drive_id, block_bitmap, block_bitmap_lba).await;

    // Initialize inode bitmap (mark reserved inodes as used)
    let mut inode_bitmap = vec![0u8; 1024].into_boxed_slice();
    // Mark first 10 inodes as used (reserved inodes)
    inode_bitmap[0] = 0xFF; // Inodes 1-8
    inode_bitmap[1] = 0x03; // Inodes 9-10
    let inode_bitmap_lba =
        entry.start_lba as i64 + ((FIRST_DATA_BLOCK + 2 + gdt_blocks) * 2) as i64;
    let _ = hal::storage::write_sectors(drive_id, inode_bitmap, inode_bitmap_lba).await;
}
