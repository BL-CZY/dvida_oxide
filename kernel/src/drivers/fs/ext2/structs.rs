use alloc::{boxed::Box, vec};
use terminal::log;

use crate::{
    drivers::fs::ext2::{
        BLOCK_GROUP_DESCRIPTOR_SIZE, GroupDescriptor, SuperBlock,
        create_file::RESERVED_BOOT_RECORD_OFFSET,
        init::identify_ext2,
        read::{INODE_BLOCK_LIMIT, INODE_IND_BLOCK_LIMIT},
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
    pub descriptor: GroupDescriptor,
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
        block_idx as i64 * self.block_size as i64 / SECTOR_SIZE as i64
    }

    pub fn lba_to_block_idx(&self, lba: i64) -> u32 {
        (lba / self.block_size as i64) as u32 * SECTOR_SIZE as u32
    }
}

#[derive(Debug)]
pub struct IoHandler {
    pub drive_id: usize,
    pub start_lba: i64,
    pub block_size: u32,
}

impl IoHandler {
    fn block_idx_to_lba(&self, block_idx: u32) -> i64 {
        block_idx as i64 * self.block_size as i64 / SECTOR_SIZE as i64
    }

    pub async fn read_sectors(
        &self,
        buffer: Box<[u8]>,
        lba: i64,
    ) -> Result<Box<[u8]>, HalStorageOperationErr> {
        storage::read_sectors(self.drive_id, buffer, self.start_lba as i64 + lba).await
    }

    pub async fn read_block(
        &self,
        buffer: Box<[u8]>,
        block_idx: u32,
    ) -> Result<Box<[u8]>, HalStorageOperationErr> {
        storage::read_sectors(
            self.drive_id,
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
            buffer,
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
        storage::write_sectors(self.drive_id, buffer, self.start_lba as i64 + lba).await
    }
}

#[derive(Debug)]
pub struct Ext2Fs {
    pub drive_id: usize,
    pub entry: GPTEntry,
    pub io_handler: IoHandler,

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
            io_handler: IoHandler {
                drive_id: drive_id,
                start_lba: entry.start_lba as i64,
                block_size: super_block.block_size(),
            },
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
        let group_number = self.lba_to_block_idx(lba) / self.super_block.s_blocks_per_group;

        self.get_group(group_number as i64).await
    }

    pub async fn get_group(&self, gr_number: i64) -> Result<Ext2BlockGroup, HalFsIOErr> {
        let bg_table_block_idx = self.super_block.s_first_data_block + 1;
        let lba = self.block_idx_to_lba(bg_table_block_idx);
        let lba_offset = (gr_number * BLOCK_GROUP_DESCRIPTOR_SIZE as i64) / SECTOR_SIZE as i64;
        let byte_offset = (gr_number * BLOCK_GROUP_DESCRIPTOR_SIZE as i64) % SECTOR_SIZE as i64;

        let mut buf: Box<[u8]> = Box::new([0u8; SECTOR_SIZE]);
        buf = self.read_sectors(buf, lba + lba_offset).await?;
        let descriptor: GroupDescriptor = *bytemuck::from_bytes(
            &buf[byte_offset as usize..byte_offset as usize + size_of::<GroupDescriptor>()],
        );

        Ok(Ext2BlockGroup {
            group_number: gr_number,
            block_size: self.super_block.block_size() as i64,
            blocks_per_group: self.super_block.s_blocks_per_group as i64,
            sectors_per_block: self.super_block.block_size() as i64 / SECTOR_SIZE as i64,
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
        let descriptor: GroupDescriptor = *bytemuck::from_bytes(&buf[byte_offset as usize..]);

        Ok(Ext2BlockGroup {
            group_number: gr_number,
            block_size: self.super_block.block_size() as i64,
            blocks_per_group: self.super_block.s_blocks_per_group as i64,
            sectors_per_block: self.super_block.block_size() as i64 / SECTOR_SIZE as i64,
            descriptor,
        })
    }

    pub fn get_block_group_table_lba(&self) -> i64 {
        let bg_table_block_idx = self.super_block.s_first_data_block + 1;
        let lba = self.block_idx_to_lba(bg_table_block_idx);
        lba
    }

    pub fn block_idx_to_lba(&self, block_idx: u32) -> i64 {
        block_idx as i64 * self.super_block.block_size() as i64 / SECTOR_SIZE as i64
    }

    pub fn lba_to_block_idx(&self, lba: i64) -> u32 {
        (lba / self.super_block.block_size() as i64) as u32 * SECTOR_SIZE as u32
    }

    pub fn get_buffer(&self) -> Box<[u8]> {
        vec![0u8; self.super_block.block_size() as usize].into()
    }
}

pub fn block_group_size(blocks_per_group: i64, block_size: i64) -> i64 {
    blocks_per_group as i64 * (block_size / SECTOR_SIZE as i64)
}

pub struct InodeBlockIterator {
    blocks: [u32; 15],
    block_size: usize,
    io_handler: IoHandler,
    cur_ind_buf: Option<Box<[u8]>>,
    cur_ind_buf_block_idx: u32,

    cur_double_ind_buf: Option<Box<[u8]>>,
    cur_double_ind_buf_block_idx: u32,

    cur_triple_ind_buf: Option<Box<[u8]>>,

    blocks_limit: usize,
    cur_idx: usize,
}

impl InodeBlockIterator {
    fn block_idx_to_lba(&self, block_idx: u32) -> i64 {
        block_idx as i64 * self.block_size as i64 / SECTOR_SIZE as i64
    }

    async fn handle_block(
        &mut self,
        mut buf: Box<[u8]>,
        block_idx: u32,
    ) -> Result<Box<[u8]>, HalFsIOErr> {
        if block_idx == 0 {
            buf.fill(0);
        } else {
            buf = self
                .io_handler
                .read_sectors(buf, self.block_idx_to_lba(block_idx))
                .await?;
        }

        Ok(buf)
    }

    async fn handle_ind_block(
        &mut self,
        mut buf: Box<[u8]>,
        block_idx: u32,
        ind_block_idx: u32,
    ) -> Result<Box<[u8]>, HalFsIOErr> {
        if self.cur_ind_buf.is_none() {
            let mut ind_buf = vec![0u8; self.block_size].into_boxed_slice();
            ind_buf = self
                .io_handler
                .read_sectors(
                    ind_buf,
                    self.block_idx_to_lba(self.blocks[INODE_BLOCK_LIMIT as usize]),
                )
                .await?;
            self.cur_ind_buf = Some(ind_buf);
        }

        Ok(buf)
    }

    pub async fn next(&mut self, mut buf: Box<[u8]>) -> Result<Option<Box<[u8]>>, HalFsIOErr> {
        if self.cur_idx >= self.blocks_limit {
            return Ok(None);
        }

        if self.cur_idx as u32 == INODE_BLOCK_LIMIT {
            self.blocks_limit -= 1;
        }

        if self.cur_idx as u32 == INODE_BLOCK_LIMIT + 1 {
            self.blocks_limit -= 1;
        }

        if self.cur_idx as u32 == INODE_BLOCK_LIMIT + 2 {
            self.blocks_limit -= 1;
        }

        if self.cur_idx >= self.blocks_limit {
            return Ok(None);
        }

        if (self.cur_idx as u32) < INODE_BLOCK_LIMIT {
            buf = self.handle_block(buf, self.blocks[self.cur_idx]).await?;
        } else if (self.cur_idx as u32) < INODE_IND_BLOCK_LIMIT {
        }

        self.cur_idx += 1;

        Ok(Some(buf))
    }
}
