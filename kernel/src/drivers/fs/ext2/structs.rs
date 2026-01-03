use alloc::{boxed::Box, vec, vec::Vec};
use terminal::log;

use crate::{
    crypto::iterators::{Bit, BitIterator},
    drivers::fs::ext2::{
        BLOCK_GROUP_DESCRIPTOR_SIZE, BLOCK_SIZE, GroupDescriptor, Inode, InodePlus, SuperBlock,
        create_file::{AllocatedBlock, RESERVED_BOOT_RECORD_OFFSET},
        init::identify_ext2,
        read::{
            INODE_BLOCK_LIMIT, INODE_DOUBLE_IND_BLOCK_LIMIT, INODE_IND_BLOCK_LIMIT,
            INODE_TRIPLE_IND_BLOCK_LIMIT,
        },
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

#[derive(Debug, Clone)]
pub struct GroupManager {
    io_handler: IoHandler,

    blocks_per_group: u32,
    first_data_block: u32,
    block_size: u32,
}

impl GroupManager {
    pub async fn get_group_from_lba(&self, lba: i64) -> Result<Ext2BlockGroup, HalFsIOErr> {
        let group_number = self.io_handler.lba_to_block_idx(lba) / self.blocks_per_group;

        self.get_group(group_number as i64).await
    }

    pub async fn get_group(&self, gr_number: i64) -> Result<Ext2BlockGroup, HalFsIOErr> {
        let bg_table_block_idx = self.first_data_block + 1;
        let lba = self.io_handler.block_idx_to_lba(bg_table_block_idx);
        let lba_offset = (gr_number * BLOCK_GROUP_DESCRIPTOR_SIZE as i64) / SECTOR_SIZE as i64;
        let byte_offset = (gr_number * BLOCK_GROUP_DESCRIPTOR_SIZE as i64) % SECTOR_SIZE as i64;

        let mut buf: Box<[u8]> = Box::new([0u8; SECTOR_SIZE]);
        buf = self.io_handler.read_sectors(buf, lba + lba_offset).await?;
        let descriptor: GroupDescriptor = *bytemuck::from_bytes(
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
        let descriptor: GroupDescriptor = *bytemuck::from_bytes(
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

#[derive(Debug, Clone)]
pub struct BlockAllocator {
    block_groups_count: i64,

    group_manager: GroupManager,
    io_handler: IoHandler,
    buffer_manager: BufferManager,
}

impl BlockAllocator {
    pub async fn allocate_n_blocks(
        &self,
        exclude_group_idx: i64,
        mut remaining_blocks: usize,
    ) -> Result<Vec<AllocatedBlock>, HalFsIOErr> {
        let mut blocks_allocated = vec![];
        let group_count = self.block_groups_count;

        // iterate over block groups
        for group_number in 0..(group_count as i64) {
            if group_number == exclude_group_idx {
                continue;
            }

            if remaining_blocks == 0 {
                break;
            }

            let group = self.group_manager.get_group(group_number as i64).await?;
            let mut buf: Box<[u8]> = Box::new([0u8; BLOCK_SIZE as usize]);

            // read block bitmap for the group
            buf = self
                .io_handler
                .read_sectors(buf, group.get_block_bitmap_lba())
                .await?;

            let bit_iterator = BitIterator::new(buf.as_mut());

            for (idx, bit) in bit_iterator.into_iter().enumerate() {
                if remaining_blocks == 0 {
                    break;
                }

                if bit == Bit::One {
                    continue;
                }

                // map bitmap index to data block LBA
                let block_lba = group.get_group_lba() + (idx as i64) * group.sectors_per_block;

                remaining_blocks -= 1;

                blocks_allocated.push(AllocatedBlock {
                    addr: block_lba,
                    block_idx: idx as i64,
                    gr_number: group_number as i64,
                });
            }
        }

        if remaining_blocks > 0 {
            return Err(HalFsIOErr::NoSpaceLeft);
        }

        Ok(blocks_allocated)
    }

    pub async fn allocate_n_blocks_in_group(
        &self,
        group_number: i64,
        mut num: usize,
    ) -> Result<Vec<AllocatedBlock>, HalFsIOErr> {
        let mut blocks_allocated = vec![];
        let group = self.group_manager.get_group(group_number).await?;
        let mut buf: Box<[u8]> = self.buffer_manager.get_buffer();

        // read the block bitmap for the group
        buf = self
            .io_handler
            .read_sectors(buf, group.get_block_bitmap_lba())
            .await?;
        let bit_iterator: BitIterator<u8> = BitIterator::new(buf.as_mut());

        for (idx, bit) in bit_iterator.into_iter().enumerate() {
            if num == 0 {
                break;
            }

            if bit == Bit::One {
                continue;
            }

            let block_lba = group.get_group_lba() + (idx as i64 * group.sectors_per_block);

            num -= 1;
            blocks_allocated.push(AllocatedBlock {
                addr: block_lba,
                block_idx: idx as i64,
                gr_number: group_number,
            });
        }

        if num == 0 {
            return Ok(blocks_allocated);
        }

        // if this group didn't satisfy the request, fall back to scanning entire fs
        blocks_allocated.extend(self.allocate_n_blocks(group_number, num).await?.into_iter());

        Ok(blocks_allocated)
    }
}

#[derive(Debug, Clone)]
pub struct BufferManager {
    block_size: usize,
}

impl BufferManager {
    pub fn get_buffer(&self) -> Box<[u8]> {
        vec![0u8; self.block_size].into()
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
            drive_id: drive_id,
            start_lba: entry.start_lba as i64,
            block_size: super_block.block_size(),
        };

        let group_manager = GroupManager {
            block_size: super_block.block_size(),
            blocks_per_group: super_block.s_blocks_per_group,
            first_data_block: super_block.s_first_data_block,
            io_handler: io_handler.clone(),
        };

        let buffer_manager = BufferManager {
            block_size: super_block.block_size() as usize,
        };

        let block_allocator = BlockAllocator {
            block_groups_count: super_block.block_groups_count() as i64,
            group_manager: group_manager.clone(),
            io_handler: io_handler.clone(),
            buffer_manager: buffer_manager.clone(),
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
        let lba = self.block_idx_to_lba(bg_table_block_idx);
        lba
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

    pub fn create_block_iterator(&self, inode: &Inode) -> InodeBlockIterator {
        InodeBlockIterator {
            blocks: inode.i_block,
            block_size: self.super_block.block_size() as usize,
            io_handler: self.io_handler.clone(),
            cur_ind_buf: None,
            cur_ind_buf_block_idx: 0,
            cur_double_ind_buf: None,
            cur_double_ind_buf_block_idx: 0,
            cur_triple_ind_buf: None,
            blocks_limit: ((inode.i_size + self.super_block.block_size() - 1)
                / self.super_block.block_size()) as usize,
            cur_idx: 0,
            cur_block_idx: 0,
        }
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

    /// will be initialized as aligned up i_size
    blocks_limit: usize,
    cur_idx: usize,
    cur_block_idx: u32,
}

impl InodeBlockIterator {
    async fn handle_block(
        &mut self,
        mut buf: Box<[u8]>,
        block_idx: u32,
    ) -> Result<Box<[u8]>, HalFsIOErr> {
        self.cur_block_idx = block_idx;
        if block_idx == 0 {
            buf.fill(0);
        } else {
            buf = self.io_handler.read_block(buf, block_idx).await?;
        }

        Ok(buf)
    }

    async fn handle_ind_block(
        &mut self,
        mut buf: Box<[u8]>,
        offset_in_ind_block: usize,
        ind_block_idx: u32,
    ) -> Result<Box<[u8]>, HalFsIOErr> {
        if ind_block_idx == 0 {
            buf.fill(0);
            return Ok(buf);
        }

        if self.cur_ind_buf.is_none() {
            let mut ind_buf = vec![0u8; self.block_size].into_boxed_slice();
            ind_buf = self
                .io_handler
                .read_block(ind_buf, self.blocks[INODE_BLOCK_LIMIT as usize])
                .await?;
            self.cur_ind_buf_block_idx = self.blocks[INODE_BLOCK_LIMIT as usize];
            self.cur_ind_buf = Some(ind_buf);
        }

        if ind_block_idx != self.cur_ind_buf_block_idx {
            let mut ind_buf = self.cur_ind_buf.take().unwrap();
            self.cur_ind_buf_block_idx = ind_block_idx;
            ind_buf = self.io_handler.read_block(ind_buf, ind_block_idx).await?;
            self.cur_ind_buf = Some(ind_buf);
        }

        let ind_buf = self.cur_ind_buf.as_ref().unwrap();
        let block_idx: u32 =
            *bytemuck::from_bytes(&ind_buf[offset_in_ind_block..offset_in_ind_block + 4]);

        Ok(self.handle_block(buf, block_idx).await?)
    }

    async fn handle_double_ind_block(
        &mut self,
        mut buf: Box<[u8]>,
        offset_in_double_ind_block: usize,
        offset_in_ind_block: usize,
        double_ind_block_idx: u32,
    ) -> Result<Box<[u8]>, HalFsIOErr> {
        if double_ind_block_idx == 0 {
            buf.fill(0);
            return Ok(buf);
        }

        if self.cur_double_ind_buf.is_none() {
            let mut double_ind_buf = vec![0u8; self.block_size].into_boxed_slice();
            double_ind_buf = self
                .io_handler
                .read_block(
                    double_ind_buf,
                    self.blocks[(INODE_BLOCK_LIMIT + 1) as usize],
                )
                .await?;
            self.cur_double_ind_buf_block_idx = self.blocks[(INODE_BLOCK_LIMIT + 1) as usize];
            self.cur_double_ind_buf = Some(double_ind_buf);
        }

        if double_ind_block_idx != self.cur_double_ind_buf_block_idx {
            let mut double_ind_buf = self.cur_double_ind_buf.take().unwrap();
            self.cur_double_ind_buf_block_idx = double_ind_block_idx;
            double_ind_buf = self
                .io_handler
                .read_block(double_ind_buf, double_ind_block_idx)
                .await?;
            self.cur_double_ind_buf = Some(double_ind_buf);
        }

        let double_ind_buf = self.cur_double_ind_buf.as_ref().unwrap();
        let ind_block_idx: u32 = *bytemuck::from_bytes(
            &double_ind_buf[offset_in_double_ind_block..offset_in_double_ind_block + 4],
        );

        Ok(self
            .handle_ind_block(buf, offset_in_ind_block, ind_block_idx)
            .await?)
    }

    pub async fn next(&mut self, buf: Box<[u8]>) -> Result<BlockIterElement, HalFsIOErr> {
        let res = self.get(buf).await?;
        self.cur_idx += 1;

        Ok(res)
    }

    pub fn skip(&mut self, count: usize) {
        self.cur_idx += count;
    }

    pub fn cur_idx(&mut self) -> usize {
        self.cur_idx
    }

    /// takes in a buffer and returns a struct BlockIterElement
    /// if the array is terminated the buffer won't be modified
    pub async fn get(&mut self, mut buf: Box<[u8]>) -> Result<BlockIterElement, HalFsIOErr> {
        if self.cur_idx >= self.blocks_limit {
            return Ok(BlockIterElement {
                buf,
                is_terminated: true,
                block_idx: 0,
            });
        }

        let num_idx_per_block = self.block_size / 4;

        if (self.cur_idx as u32) < INODE_BLOCK_LIMIT {
            buf = self.handle_block(buf, self.blocks[self.cur_idx]).await?;
        } else if (self.cur_idx as u32) < INODE_IND_BLOCK_LIMIT {
            buf = self
                .handle_ind_block(
                    buf,
                    self.cur_idx % INODE_BLOCK_LIMIT as usize,
                    self.blocks[INODE_BLOCK_LIMIT as usize],
                )
                .await?;
        } else if (self.cur_idx as u32) < INODE_DOUBLE_IND_BLOCK_LIMIT {
            let offset_in_ind_block =
                ((self.cur_idx - INODE_IND_BLOCK_LIMIT as usize) % num_idx_per_block as usize) * 4;
            let offset_in_double_ind_block =
                ((self.cur_idx - INODE_IND_BLOCK_LIMIT as usize) / num_idx_per_block as usize) * 4;
            buf = self
                .handle_double_ind_block(
                    buf,
                    offset_in_double_ind_block,
                    offset_in_ind_block,
                    self.blocks[INODE_BLOCK_LIMIT as usize + 1],
                )
                .await?;
        } else if (self.cur_idx as u32) < INODE_TRIPLE_IND_BLOCK_LIMIT {
            if self.blocks[INODE_BLOCK_LIMIT as usize + 2] == 0 {
                buf.fill(0);
            } else {
                if self.cur_triple_ind_buf.is_none() {
                    let mut triple_ind_buf = vec![0u8; self.block_size].into_boxed_slice();
                    triple_ind_buf = self
                        .io_handler
                        .read_block(
                            triple_ind_buf,
                            self.blocks[(INODE_BLOCK_LIMIT + 2) as usize],
                        )
                        .await?;
                    self.cur_triple_ind_buf = Some(triple_ind_buf);
                }

                let triple_ind_buf = self.cur_triple_ind_buf.as_ref().unwrap();
                let offset_in_ind_block = ((self.cur_idx - INODE_DOUBLE_IND_BLOCK_LIMIT as usize)
                    % num_idx_per_block as usize)
                    * 4;

                let offset_in_double_ind_block = (((self.cur_idx
                    - INODE_DOUBLE_IND_BLOCK_LIMIT as usize)
                    / num_idx_per_block as usize)
                    % num_idx_per_block as usize)
                    * 4;

                let offset_in_triple_ind_block = ((self.cur_idx
                    - INODE_DOUBLE_IND_BLOCK_LIMIT as usize)
                    / (num_idx_per_block * num_idx_per_block) as usize)
                    * 4;

                let double_ind_block_idx: u32 = *bytemuck::from_bytes(
                    &triple_ind_buf[offset_in_triple_ind_block..offset_in_triple_ind_block + 4],
                );

                buf = self
                    .handle_double_ind_block(
                        buf,
                        offset_in_double_ind_block,
                        offset_in_ind_block,
                        double_ind_block_idx,
                    )
                    .await?;
            }
        } else {
            return Ok(BlockIterElement {
                buf,
                is_terminated: true,
                block_idx: 0,
            });
        }

        Ok(BlockIterElement {
            buf: buf,
            is_terminated: false,
            block_idx: self.cur_block_idx,
        })
    }

    pub async fn set<T>(&mut self, allocate_block: T) -> Result<(), HalFsIOErr>
    where
        T: AsyncFnOnce(usize) -> Result<Vec<AllocatedBlock>, HalFsIOErr>,
    {
        todo!()
    }
}

pub struct BlockIterElement {
    pub buf: Box<[u8]>,
    pub is_terminated: bool,
    /// if the array is not terminated it will contain the block index of the block, else the value
    /// is undefined
    pub block_idx: u32,
}
