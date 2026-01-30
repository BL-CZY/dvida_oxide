use crate::ejcineque::sync::mutex::Mutex;
use crate::log;
use alloc::{boxed::Box, collections::btree_set::BTreeSet, sync::Arc};

use crate::{
    drivers::fs::ext2::{
        GroupDescriptor, SuperBlock, create_file::RESERVED_BOOT_RECORD_OFFSET, init::identify_ext2,
    },
    hal::{
        fs::HalFsIOErr,
        gpt::GPTEntry,
        storage::{HalStorageOperationErr, SECTOR_SIZE},
    },
};

pub use super::allocator::BlockAllocator;
pub use super::block_iterator::{BlockIterElement, InodeBlockIterator};
pub use super::managers::{BufferManager, GroupManager, IoHandler};

/// no sparse superblock
#[derive(Debug)]
pub struct Ext2BlockGroup {
    pub group_number: i64,
    pub block_size: i64,
    pub blocks_per_group: i64,
    pub sectors_per_block: i64,
    pub descriptor: GroupDescriptor,
}

impl Ext2BlockGroup {
    pub fn get_group_lba(&self) -> i64 {
        if self.group_number == 0 {
            RESERVED_BOOT_RECORD_OFFSET
        } else {
            RESERVED_BOOT_RECORD_OFFSET
                + ((self.blocks_per_group - 1) * self.sectors_per_block)
                + (1024 / SECTOR_SIZE as i64)
                + (self.group_number - 1) * self.blocks_per_group * self.sectors_per_block
        }
    }

    pub fn get_superblock_lba(&self) -> Option<i64> {
        Some(self.get_group_lba())
    }

    pub fn get_blockgroup_desc_table_lba(&self) -> Option<i64> {
        if self.group_number == 0 {
            Some(self.get_group_lba() + 1024 / SECTOR_SIZE as i64)
        } else {
            Some(self.get_group_lba() + self.sectors_per_block)
        }
    }

    pub fn get_block_bitmap_lba(&self) -> i64 {
        self.block_idx_to_lba(self.descriptor.bg_block_bitmap)
    }

    pub fn get_inode_bitmap_lba(&self) -> i64 {
        self.block_idx_to_lba(self.descriptor.bg_inode_bitmap)
    }

    pub fn get_inode_table_lba(&self) -> i64 {
        self.block_idx_to_lba(self.descriptor.bg_inode_table)
    }

    pub fn get_data_blocks_start_lba(&self) -> i64 {
        self.get_group_lba()
    }

    pub fn block_idx_to_lba(&self, block_idx: u32) -> i64 {
        block_idx as i64 * self.block_size / SECTOR_SIZE as i64
    }

    pub fn lba_to_block_idx(&self, lba: i64) -> u32 {
        (lba / self.block_size) as u32 * SECTOR_SIZE as u32
    }
}

#[derive(Debug, Clone)]
pub struct Ext2Fs {
    pub drive_id: usize,
    pub entry: GPTEntry,
    pub io_handler: IoHandler,
    pub block_allocator: BlockAllocator,
    pub group_manager: GroupManager,
    pub buffer_manager: BufferManager,

    pub super_block: SuperBlock,
}

impl Ext2Fs {
    pub async fn new(drive_id: usize, entry: GPTEntry) -> Self {
        let super_block = identify_ext2(drive_id, &entry)
            .await
            .expect("Failed to mount ext2");

        log!("Mounted ext2");

        let io_handler = IoHandler {
            drive_id,
            start_lba: entry.start_lba as i64,
            block_size: super_block.block_size(),
        };

        let group_manager = GroupManager {
            block_size: super_block.block_size(),
            blocks_per_group: super_block.s_blocks_per_group,
            first_data_block: super_block.s_first_data_block,
            io_handler,
        };

        let buffer_manager = BufferManager {
            block_size: super_block.block_size() as usize,
        };

        let block_allocator = BlockAllocator {
            block_groups_count: super_block.block_groups_count() as i64,
            group_manager,
            io_handler,
            buffer_manager,
            allocated_block_indices: Arc::new(Mutex::new(BTreeSet::new())),
            unwritten_freed_blocks: Arc::new(Mutex::new(BTreeSet::new())),
        };

        Self {
            drive_id,
            io_handler,
            group_manager,
            block_allocator,
            buffer_manager,
            entry,
            super_block,
        }
    }

    /// relative LBA
    pub async fn read_sectors(
        &self,
        buffer: Box<[u8]>,
        lba: i64,
    ) -> Result<Box<[u8]>, HalStorageOperationErr> {
        self.io_handler.read_sectors(buffer, lba).await
    }

    // relative LBA
    pub async fn write_sectors(
        &self,
        buffer: Box<[u8]>,
        lba: i64,
    ) -> Result<(), HalStorageOperationErr> {
        self.io_handler.write_sectors(buffer, lba).await
    }

    pub fn len(&self) -> i64 {
        (self.entry.end_lba - self.entry.start_lba)
            .try_into()
            .unwrap_or(i64::MAX)
    }

    pub async fn get_group_from_lba(&self, lba: i64) -> Result<Ext2BlockGroup, HalFsIOErr> {
        self.group_manager.get_group_from_lba(lba).await
    }

    pub async fn get_group(&self, gr_number: i64) -> Result<Ext2BlockGroup, HalFsIOErr> {
        self.group_manager.get_group(gr_number).await
    }

    /// parses a block group from a buffer
    /// will assume the buf's size to be BLOCK_SIZE and use
    /// ```
    /// let byte_offset = (gr_number * BLOCK_GROUP_DESCRIPTOR_SIZE as i64) % SECTOR_SIZE asi64;
    /// ```
    /// to index into the buffer
    pub fn get_group_from_buffer(
        &self,
        gr_number: i64,
        buf: &Box<[u8]>,
    ) -> Result<Ext2BlockGroup, HalFsIOErr> {
        self.group_manager.get_group_from_buffer(gr_number, buf)
    }

    pub fn get_block_group_table_lba(&self) -> i64 {
        let bg_table_block_idx = self.super_block.s_first_data_block + 1;
        
        self.block_idx_to_lba(bg_table_block_idx)
    }

    pub fn block_idx_to_lba(&self, block_idx: u32) -> i64 {
        self.io_handler.block_idx_to_lba(block_idx)
    }

    pub fn lba_to_block_idx(&self, lba: i64) -> u32 {
        self.io_handler.lba_to_block_idx(lba)
    }

    pub fn get_buffer(&self) -> Box<[u8]> {
        self.buffer_manager.get_buffer()
    }
}

pub fn block_group_size(blocks_per_group: i64, block_size: i64) -> i64 {
    blocks_per_group * (block_size / SECTOR_SIZE as i64)
}
