use core::error::Error;

use alloc::string::ToString;
use alloc::vec;
use alloc::{boxed::Box, vec::Vec};
use dvida_serialize::{DvDeserialize, DvSerialize, Endianness};
use terminal::log;

use crate::drivers::fs::ext2::INODES_PER_GROUP;
use crate::hal::storage::write_sectors;
use crate::{
    crypto::uuid::uuid_v4,
    drivers::fs::ext2::{
        ALGO_BITMAP, BLOCK_SIZE, BLOCKS_PER_GROUP, CREATOR_OS_DVIDA, DirEntry,
        EXT2_ERRORS_CONTINUE, EXT2_ROOT_INO, EXT2_S_IFDIR, EXT2_SUPER_MAGIC, EXT2_VALID_FS,
        GroupDescriptor, Inode, LOG_BLOCK_SIZE, MAX_MOUNT_COUNT, ROOT_ID, SuperBlock,
    },
    hal::{gpt::GPTEntry, storage::read_sectors},
    time::{Rtc, RtcDateTime, formats::rtc_to_posix},
};

pub async fn identify_ext2(drive_id: usize, entry: &GPTEntry) -> Option<SuperBlock> {
    let mut buf = Box::new([0u8; 1024]);

    if entry.start_lba - entry.end_lba < 3 {
        log!("Failed to identify ext2 because the GPT entry is too small");
        return None;
    }

    match read_sectors(drive_id, buf.clone(), (entry.start_lba + 1) as i64).await {
        Ok(_) => {}
        Err(err) => {
            log!("Failed to identify ext2 because of read error: {}", err);
            return None;
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
                return None;
            }
        };

    if super_block.s_magic == 0xEF53 {
        Some(super_block)
    } else {
        None
    }
}

// ext2 structure sizes (in bytes) according to spec
const GROUP_DESCRIPTOR_SIZE: usize = 32;
const INODE_SIZE: u16 = 128;
const FIRST_DATA_BLOCK_ADDR: u32 = 1024 / BLOCK_SIZE;

// blocksize is considered to be 1kb
pub async fn init_ext2(drive_id: usize, entry: &GPTEntry) -> Result<(), Box<dyn Error>> {
    // TODO: check entry eligibility

    let time = rtc_to_posix(&Rtc::new().read_datetime().expect("No time acquired"));

    let mut free_blocks_count: u32 = 0;
    let mut free_inodes_count: u32 = 0;

    // Number of sectors in the partition
    let num_sectors = (entry.end_lba as i64 - entry.start_lba as i64 + 1) as u64;
    let sectors_per_block = (BLOCK_SIZE / 512) as u64;

    // Number of blocks (ceil)
    let blocks = (num_sectors + sectors_per_block - 1) / sectors_per_block;

    // Number of block groups (ceil)
    let num_block_groups =
        ((blocks + BLOCKS_PER_GROUP as u64 - 1) / BLOCKS_PER_GROUP as u64) as usize;

    let mut bg_vec: Vec<GroupDescriptor> = vec![];

    // Partition start in blocks
    let partition_start_block = (entry.start_lba as u64) / sectors_per_block;

    for i in 0..num_block_groups {
        // group start block (absolute, relative to partition)
        let group_start_block = partition_start_block + i as u64 * BLOCKS_PER_GROUP as u64;

        // inode table size in blocks
        let inode_table_blocks = (((INODE_SIZE as u32) * INODES_PER_GROUP as u32)
            + (BLOCK_SIZE as u16 as u32 - 1))
            / BLOCK_SIZE as u32;

        // blocks reserved for metadata in a group: block bitmap + inode bitmap + inode table
        let reserved_blocks = 1u32 + 1u32 + inode_table_blocks;

        let bg_descriptor = GroupDescriptor {
            bg_block_bitmap: (group_start_block + 1) as u32,
            bg_inode_bitmap: (group_start_block + 2) as u32,
            bg_inode_table: (group_start_block + 3) as u32,
            bg_free_blocks_count: (BLOCKS_PER_GROUP as u32 - reserved_blocks) as u16,
            bg_free_inodes_count: INODES_PER_GROUP as u16,
            bg_used_dirs_count: 0,
        };

        free_blocks_count += bg_descriptor.bg_free_blocks_count as u32;
        free_inodes_count += bg_descriptor.bg_free_inodes_count as u32;
        bg_vec.push(bg_descriptor);
    }

    let mut super_block: SuperBlock = SuperBlock {
        // total inodes and blocks across filesystem
        s_inodes_count: (INODES_PER_GROUP as u32 * num_block_groups as u32),
        s_blocks_count: blocks as u32,
        s_r_blocks_count: 0,
        s_free_blocks_count: free_blocks_count,
        s_free_inodes_count: free_inodes_count,
        s_first_data_block: FIRST_DATA_BLOCK_ADDR,
        s_log_block_size: LOG_BLOCK_SIZE,
        s_log_frag_size: LOG_BLOCK_SIZE, // this is not supported
        s_blocks_per_group: BLOCKS_PER_GROUP,
        s_frags_per_group: BLOCKS_PER_GROUP,
        s_inodes_per_group: INODES_PER_GROUP,
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

        s_first_ino: 11,
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

    // Prepare buffers and write structures per block group.
    // Note: this is a simplified layout writer — a production implementation
    // must follow the ext2 spec closely (sparse superblocks, alignment, etc.).

    // serialized superblock buffer (one block)
    let mut sb_buf = vec![0u8; BLOCK_SIZE as usize];

    // group descriptor table buffer (rounded up to block size)
    let gd_bytes = (bg_vec.len() * GROUP_DESCRIPTOR_SIZE) as usize;
    let gd_blocks = (gd_bytes + BLOCK_SIZE as usize - 1) / BLOCK_SIZE as usize;
    let mut gd_buf = vec![0u8; gd_blocks * BLOCK_SIZE as usize];

    // inode table zero buffer (we'll write per-group inode table blocks)

    for bg_idx in 0..bg_vec.len() {
        // prepare buffers for this group
        sb_buf.fill(0);
        super_block.serialize(Endianness::Little, &mut sb_buf)?;

        // write zeroed inode table blocks for this group
        let inode_table_blocks = (((INODE_SIZE as u32) * INODES_PER_GROUP as u32)
            + (BLOCK_SIZE as u16 as u32 - 1))
            / BLOCK_SIZE as u32;

        let inode_buf_len = (inode_table_blocks as usize) * BLOCK_SIZE as usize;
        let mut inode_buf = vec![0u8; inode_buf_len];

        // If this is group 0, create the root inode and its directory block
        if bg_idx == 0 {
            // compute some helpful values
            let group_start_block = partition_start_block + bg_idx as u64 * BLOCKS_PER_GROUP as u64;
            let data_blocks_start = group_start_block + 3 + inode_table_blocks as u64;

            // data block absolute (relative to partition)
            let root_data_block = data_blocks_start; // use first data block in group

            // prepare directory block with "." and ".." entries
            let mut dir_block = vec![0u8; BLOCK_SIZE as usize];
            let dot = DirEntry::new(EXT2_ROOT_INO, ".".to_string());
            let dotdot = DirEntry::new(EXT2_ROOT_INO, "..".to_string());

            // serialize '.' at offset 0
            let first_len = dot.serialize(Endianness::Little, &mut dir_block[0..])?;

            // serialize '..' at offset first_len
            let second_offset = first_len;
            let _second_len =
                dotdot.serialize(Endianness::Little, &mut dir_block[second_offset..])?;

            // adjust the rec_len of the second entry to fill the rest of the block
            let final_rec_len: u16 = (BLOCK_SIZE as u16) - (first_len as u16);
            // rec_len is at offset second_offset + 4 (after inode u32)
            dir_block[second_offset + 4] = (final_rec_len & 0xFF) as u8;
            dir_block[second_offset + 5] = ((final_rec_len >> 8) & 0xFF) as u8;

            // write directory block to disk
            let data_sector = entry.start_lba as u64 + (root_data_block * sectors_per_block);
            write_sectors(drive_id, dir_block.into_boxed_slice(), data_sector as i64).await?;

            // construct root inode and write into inode table buffer
            let root_inode_index = EXT2_ROOT_INO as u32;
            let relative_idx = (root_inode_index - 1) as usize; // zero-based

            let root_inode = Inode {
                i_mode: (EXT2_S_IFDIR | 0o755) as u16,
                i_uid: 0,
                i_size: BLOCK_SIZE,
                i_atime: time,
                i_ctime: time,
                i_mtime: time,
                i_dtime: 0,
                i_gid: 0,
                i_links_count: 2,
                i_blocks: 1,
                i_flags: 0,
                i_osd1: 0,
                i_block: {
                    let mut a = [0u32; 15];
                    a[0] = root_data_block as u32 + 0; // block number relative to partition
                    a
                },
                i_generation: 0,
                i_file_acl: 0,
                i_dir_acl: 0,
                i_faddr: 0,
                i_osd2: [0u8; 12],
            };

            // serialize root inode into inode_buf at proper offset
            let inode_offset = relative_idx * INODE_SIZE as usize;
            root_inode.serialize(Endianness::Little, &mut inode_buf[inode_offset..])?;

            // adjust counts in descriptors/superblock
            {
                let bgd = &mut bg_vec[bg_idx];
                bgd.bg_free_inodes_count = bgd.bg_free_inodes_count.saturating_sub(1);
                bgd.bg_free_blocks_count = bgd.bg_free_blocks_count.saturating_sub(1);
                bgd.bg_used_dirs_count = bgd.bg_used_dirs_count.saturating_add(1);
            }

            super_block.s_free_inodes_count = super_block.s_free_inodes_count.saturating_sub(1);
            super_block.s_free_blocks_count = super_block.s_free_blocks_count.saturating_sub(1);
        }
        // prepare and write full group descriptor table at the group's descriptor location
        gd_buf.fill(0);
        for (idx, bgd) in bg_vec.iter().enumerate() {
            bgd.serialize(
                Endianness::Little,
                &mut gd_buf[idx * GROUP_DESCRIPTOR_SIZE..],
            )?;
        }

        let gd_sector = entry.start_lba as u64
            + ((bg_idx as u64 * BLOCKS_PER_GROUP as u64 + 2) * sectors_per_block);

        write_sectors(
            drive_id,
            gd_buf.clone().into_boxed_slice(),
            gd_sector as i64,
        )
        .await?;

        // compute absolute sector for this group's superblock copy
        let superblock_sector = entry.start_lba as u64
            + ((bg_idx as u64 * BLOCKS_PER_GROUP as u64 + 1) * sectors_per_block);

        write_sectors(
            drive_id,
            sb_buf.clone().into_boxed_slice(),
            superblock_sector as i64,
        )
        .await?;

        let inode_table_sector = entry.start_lba as u64
            + ((bg_idx as u64 * BLOCKS_PER_GROUP as u64 + 3) * sectors_per_block);

        write_sectors(
            drive_id,
            inode_buf.into_boxed_slice(),
            inode_table_sector as i64,
        )
        .await?;
    }

    Ok(())
}
