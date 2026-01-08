use alloc::boxed::Box;

use crate::{
    drivers::fs::ext2::{BLOCK_GROUP_DESCRIPTOR_SIZE, GroupDescriptor, structs::Ext2BlockGroup},
    hal::{
        fs::HalFsIOErr,
        storage::{self, HalStorageOperationErr, SECTOR_SIZE},
    },
};
use alloc::vec;

#[derive(Debug, Clone, Copy)]
pub struct IoHandler {
    pub drive_id: usize,
    pub start_lba: i64,
    pub block_size: u32,
}

impl IoHandler {
    pub fn block_idx_to_lba(&self, block_idx: u32) -> i64 {
        block_idx as i64 * self.block_size as i64 / SECTOR_SIZE as i64
    }

    pub fn lba_to_block_idx(&self, lba: i64) -> u32 {
        (lba / self.block_size as i64) as u32 * SECTOR_SIZE as u32
    }

    pub async fn read_sectors(
        &self,
        buffer: Box<[u8]>,
        lba: i64,
    ) -> Result<Box<[u8]>, HalStorageOperationErr> {
        storage::read_sectors(self.drive_id, buffer.into(), self.start_lba as i64 + lba)
            .await
            .map(|b| b.into())
    }

    pub async fn read_block(
        &self,
        buffer: Box<[u8]>,
        block_idx: u32,
    ) -> Result<Box<[u8]>, HalStorageOperationErr> {
        self.read_sectors(
            buffer,
            self.start_lba as i64 + self.block_idx_to_lba(block_idx),
        )
        .await
    }

    pub async fn write_block(
        &self,
        buffer: Box<[u8]>,
        block_idx: u32,
    ) -> Result<(), HalStorageOperationErr> {
        storage::write_sectors(
            self.drive_id,
            buffer.into(),
            self.start_lba as i64 + self.block_idx_to_lba(block_idx),
        )
        .await
    }

    // relative LBA
    pub async fn write_sectors(
        &self,
        buffer: Box<[u8]>,
        lba: i64,
    ) -> Result<(), HalStorageOperationErr> {
        storage::write_sectors(self.drive_id, buffer.into(), self.start_lba as i64 + lba).await
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GroupManager {
    pub io_handler: IoHandler,

    pub blocks_per_group: u32,
    pub first_data_block: u32,
    pub block_size: u32,
}

impl GroupManager {
    pub async fn get_group_from_lba(&self, lba: i64) -> Result<Ext2BlockGroup, HalFsIOErr> {
        let group_number = self.io_handler.lba_to_block_idx(lba) / self.blocks_per_group;

        self.get_group(group_number as i64).await
    }

    pub async fn get_group_from_block_idx(&self, idx: u32) -> Result<Ext2BlockGroup, HalFsIOErr> {
        let group_number = idx / self.blocks_per_group;

        self.get_group(group_number as i64).await
    }

    pub async fn get_group(&self, gr_number: i64) -> Result<Ext2BlockGroup, HalFsIOErr> {
        let bg_table_block_idx = self.first_data_block + 1;
        let lba = self.io_handler.block_idx_to_lba(bg_table_block_idx);
        let lba_offset = (gr_number * BLOCK_GROUP_DESCRIPTOR_SIZE as i64) / SECTOR_SIZE as i64;
        let byte_offset = (gr_number * BLOCK_GROUP_DESCRIPTOR_SIZE as i64) % SECTOR_SIZE as i64;

        let mut buf: Box<[u8]> = Box::new([0u8; SECTOR_SIZE]);
        buf = self.io_handler.read_sectors(buf, lba + lba_offset).await?;
        let descriptor: super::GroupDescriptor = *bytemuck::from_bytes(
            &buf[byte_offset as usize..byte_offset as usize + size_of::<GroupDescriptor>()],
        );

        Ok(Ext2BlockGroup {
            group_number: gr_number,
            block_size: self.block_size as i64,
            blocks_per_group: self.blocks_per_group as i64,
            sectors_per_block: self.block_size as i64 / SECTOR_SIZE as i64,
            descriptor,
        })
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
        let byte_offset = (gr_number * BLOCK_GROUP_DESCRIPTOR_SIZE as i64) % SECTOR_SIZE as i64;
        let descriptor: super::GroupDescriptor = *bytemuck::from_bytes(
            &buf[byte_offset as usize..byte_offset as usize + size_of::<GroupDescriptor>()],
        );

        Ok(Ext2BlockGroup {
            group_number: gr_number,
            block_size: self.block_size as i64,
            blocks_per_group: self.blocks_per_group as i64,
            sectors_per_block: self.block_size as i64 / SECTOR_SIZE as i64,
            descriptor,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BufferManager {
    pub block_size: usize,
}

impl BufferManager {
    pub fn get_buffer(&self) -> Box<[u8]> {
        vec![0u8; self.block_size].into()
    }
}
