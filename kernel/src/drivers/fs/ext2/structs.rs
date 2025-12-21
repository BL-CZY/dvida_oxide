use alloc::{boxed::Box, vec::Vec};

use crate::{
    drivers::fs::ext2::{
        GroupDescriptor, SuperBlock,
        create_file::{BLOCK_SECTOR_SIZE, RESERVED_BOOT_RECORD_OFFSET},
    },
    hal::{
        gpt::GPTEntry,
        storage::{self, HalStorageOperationErr, SECTOR_SIZE},
    },
};

/// no sparse superblock
#[derive(Debug)]
pub struct Ext2BlockGroup {
    pub group_number: i64,
    pub blocks_per_group: i64,
    pub sectors_per_block: i64,
}

impl Ext2BlockGroup {
    pub fn get_group_lba(&self) -> i64 {
        if self.group_number == 0 {
            0 + RESERVED_BOOT_RECORD_OFFSET
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
        if self.group_number == 0 {
            self.get_group_lba() + 1024 / SECTOR_SIZE as i64 + self.sectors_per_block
        } else {
            self.get_group_lba() + self.sectors_per_block * 2
        }
    }

    pub fn get_inode_bitmap_lba(&self) -> i64 {
        if self.group_number == 0 {
            self.get_group_lba() + 1024 / SECTOR_SIZE as i64 + self.sectors_per_block * 2
        } else {
            self.get_group_lba() + self.sectors_per_block * 3
        }
    }

    pub fn get_inode_table_lba(&self) -> i64 {
        if self.group_number == 0 {
            self.get_group_lba() + 1024 / SECTOR_SIZE as i64 + self.sectors_per_block * 3
        } else {
            self.get_group_lba() + self.sectors_per_block * 4
        }
    }

    pub fn get_data_blocks_start_lba(&self) -> i64 {
        if self.group_number == 0 {
            self.get_group_lba() + 1024 / SECTOR_SIZE as i64 + self.sectors_per_block * 217
        } else {
            self.get_group_lba() + self.sectors_per_block * 218
        }
    }
}

#[derive(Debug)]
pub struct Ext2Fs {
    pub drive_id: usize,
    pub entry: GPTEntry,

    pub super_block: SuperBlock,
    pub group_descs: Vec<GroupDescriptor>,
}

impl Ext2Fs {
    /// relative LBA
    pub async fn read_sectors(
        &self,
        buffer: Box<[u8]>,
        lba: i64,
    ) -> Result<(), HalStorageOperationErr> {
        storage::read_sectors(self.drive_id, buffer, self.entry.start_lba as i64 + lba).await
    }

    // relative LBA
    pub async fn write_sectors(
        &self,
        buffer: Box<[u8]>,
        lba: i64,
    ) -> Result<(), HalStorageOperationErr> {
        storage::write_sectors(self.drive_id, buffer, self.entry.start_lba as i64 + lba).await
    }

    pub fn len(&self) -> i64 {
        (self.entry.end_lba - self.entry.start_lba)
            .try_into()
            .unwrap_or(i64::MAX)
    }

    pub fn get_group_from_lba(&self, lba: i64) -> Ext2BlockGroup {
        Ext2BlockGroup {
            group_number: lba
                / (self.super_block.s_blocks_per_group as i64 * BLOCK_SECTOR_SIZE) as i64,
            blocks_per_group: self.super_block.s_blocks_per_group as i64,
            sectors_per_block: self.super_block.block_size() as i64 / SECTOR_SIZE as i64,
        }
    }

    pub fn get_group(&self, gr_number: i64) -> Ext2BlockGroup {
        Ext2BlockGroup {
            group_number: gr_number,
            blocks_per_group: self.super_block.s_blocks_per_group as i64,
            sectors_per_block: self.super_block.block_size() as i64 / SECTOR_SIZE as i64,
        }
    }
}

pub fn block_group_size(blocks_per_group: i64, block_size: i64) -> i64 {
    blocks_per_group as i64 * (block_size / SECTOR_SIZE as i64)
}
