use alloc::boxed::Box;
use dvida_serialize::DvDeserialize;
use terminal::log;

use crate::{
    drivers::fs::ext2::{
        BLOCK_GROUP_DESCRIPTOR_SIZE, GroupDescriptorPartial, SuperBlock,
        create_file::RESERVED_BOOT_RECORD_OFFSET, init::identify_ext2,
    },
    hal::{
        fs::HalFsIOErr,
        gpt::GPTEntry,
        storage::{self, HalStorageOperationErr, SECTOR_SIZE},
    },
};

/// no sparse superblock
#[derive(Debug)]
pub struct Ext2BlockGroup {
    pub group_number: i64,
    pub block_size: i64,
    pub blocks_per_group: i64,
    pub sectors_per_block: i64,
    pub descriptor_partial: GroupDescriptorPartial,
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
        self.block_idx_to_lba(self.descriptor_partial.bg_block_bitmap)
    }

    pub fn get_inode_bitmap_lba(&self) -> i64 {
        self.block_idx_to_lba(self.descriptor_partial.bg_inode_bitmap)
    }

    pub fn get_inode_table_lba(&self) -> i64 {
        self.block_idx_to_lba(self.descriptor_partial.bg_inode_table)
    }

    pub fn get_data_blocks_start_lba(&self) -> i64 {
        self.get_group_lba()
    }

    pub fn block_idx_to_lba(&self, block_idx: u32) -> i64 {
        block_idx as i64 * self.block_size as i64 / SECTOR_SIZE as i64
    }

    pub fn lba_to_block_idx(&self, lba: i64) -> u32 {
        (lba / self.block_size as i64) as u32 * SECTOR_SIZE as u32
    }
}

#[derive(Debug)]
pub struct Ext2Fs {
    pub drive_id: usize,
    pub entry: GPTEntry,

    pub super_block: SuperBlock,
}

impl Ext2Fs {
    pub async fn new(drive_id: usize, entry: GPTEntry) -> Self {
        let super_block = identify_ext2(drive_id, &entry)
            .await
            .expect("Failed to mount ext2");

        log!("Mounted ext2");

        Self {
            drive_id,
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

    pub async fn get_group_from_lba(&self, lba: i64) -> Result<Ext2BlockGroup, HalFsIOErr> {
        let group_number = self.lba_to_block_idx(lba) / self.super_block.s_blocks_per_group;

        self.get_group(group_number as i64).await
    }

    pub async fn get_group(&self, gr_number: i64) -> Result<Ext2BlockGroup, HalFsIOErr> {
        let bg_table_block_idx = self.super_block.s_first_data_block;
        let lba = self.block_idx_to_lba(bg_table_block_idx);
        let lba_offset = (gr_number * BLOCK_GROUP_DESCRIPTOR_SIZE as i64) / SECTOR_SIZE as i64;
        let byte_offset = (gr_number * BLOCK_GROUP_DESCRIPTOR_SIZE as i64) % SECTOR_SIZE as i64;

        let mut buf: Box<[u8]> = Box::new([0u8; SECTOR_SIZE]);
        buf = self.read_sectors(buf, lba + lba_offset).await?;

        Ok(Ext2BlockGroup {
            group_number: gr_number,
            block_size: self.super_block.block_size() as i64,
            blocks_per_group: self.super_block.s_blocks_per_group as i64,
            sectors_per_block: self.super_block.block_size() as i64 / SECTOR_SIZE as i64,
            descriptor_partial: GroupDescriptorPartial::deserialize(
                dvida_serialize::Endianness::Little,
                &buf[byte_offset as usize..],
            )?
            .0,
        })
    }

    pub fn block_idx_to_lba(&self, block_idx: u32) -> i64 {
        block_idx as i64 * self.super_block.block_size() as i64 / SECTOR_SIZE as i64
    }

    pub fn lba_to_block_idx(&self, lba: i64) -> u32 {
        (lba / self.super_block.block_size() as i64) as u32 * SECTOR_SIZE as u32
    }
}

pub fn block_group_size(blocks_per_group: i64, block_size: i64) -> i64 {
    blocks_per_group as i64 * (block_size / SECTOR_SIZE as i64)
}
