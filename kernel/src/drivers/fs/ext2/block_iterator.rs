use alloc::boxed::Box;
use alloc::{vec, vec::Vec};
use terminal::log;

use crate::drivers::fs::ext2::Inode;
use crate::drivers::fs::ext2::structs::{BlockAllocator, Ext2Fs};
use crate::{
    drivers::fs::ext2::{
        create_file::AllocatedBlock,
        read::{
            INODE_BLOCK_LIMIT, INODE_DOUBLE_IND_BLOCK_LIMIT, INODE_IND_BLOCK_LIMIT,
            INODE_TRIPLE_IND_BLOCK_LIMIT,
        },
        structs::IoHandler,
    },
    hal::fs::HalFsIOErr,
};

impl Ext2Fs {
    pub fn create_block_iterator(&self, inode: &Inode, group_number: i64) -> InodeBlockIterator {
        InodeBlockIterator {
            blocks: inode.i_block,
            group_number,

            block_size: self.super_block.block_size() as usize,
            io_handler: self.io_handler,
            block_allocator: self.block_allocator.clone(),

            cur_ind_buf: None,
            cur_ind_buf_block_idx: 0,
            cur_double_ind_buf: None,
            cur_double_ind_buf_block_idx: 0,
            cur_triple_ind_buf: None,
            cur_triple_ind_buf_block_idx: 0,
            blocks_limit: ((inode.i_size + self.super_block.block_size() - 1)
                / self.super_block.block_size()) as usize,
            cur_idx: 0,
            cur_block_idx: 0,
        }
    }
}

pub struct InodeBlockIterator {
    blocks: [u32; 15],
    group_number: i64,

    block_size: usize,
    io_handler: IoHandler,
    block_allocator: BlockAllocator,

    cur_ind_buf: Option<Box<[u8]>>,
    cur_ind_buf_block_idx: u32,

    cur_double_ind_buf: Option<Box<[u8]>>,
    cur_double_ind_buf_block_idx: u32,

    cur_triple_ind_buf: Option<Box<[u8]>>,
    cur_triple_ind_buf_block_idx: u32,

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
        if self.cur_idx >= self.blocks_limit {
            return Ok(BlockIterElement {
                buf,
                is_terminated: true,
                block_idx: 0,
            });
        }

        let res = self.get(buf).await?;
        self.cur_idx += 1;

        Ok(res)
    }

    pub async fn next_set(&mut self) -> Result<BlockIterSetRes, HalFsIOErr> {
        let res = self.set().await?;
        self.cur_idx += 1;

        Ok(res)
    }

    pub fn skip(&mut self, count: usize) {
        self.cur_idx += count;
    }

    pub fn skip_to_end(&mut self) {
        self.cur_idx = self.blocks_limit;
    }

    pub fn cur_idx(&mut self) -> usize {
        self.cur_idx
    }

    /// takes in a buffer and returns a struct BlockIterElement
    /// if the array is terminated the buffer won't be modified
    pub async fn get(&mut self, mut buf: Box<[u8]>) -> Result<BlockIterElement, HalFsIOErr> {
        let num_idx_per_block = self.block_size / 4;

        if (self.cur_idx as u32) < INODE_BLOCK_LIMIT {
            buf = self.handle_block(buf, self.blocks[self.cur_idx]).await?;
        } else if (self.cur_idx as u32) < INODE_IND_BLOCK_LIMIT {
            buf = self
                .handle_ind_block(
                    buf,
                    self.cur_idx - INODE_BLOCK_LIMIT as usize,
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

    async fn handle_set_block(
        &mut self,
        // guaranteed to not be 0
        ind_block_idx: u32,
        offset_in_ind_block: usize,
        allocated_blocks: &mut Vec<AllocatedBlock>,
    ) -> Result<(), HalFsIOErr> {
        if self.cur_ind_buf_block_idx != ind_block_idx {
            let mut buf = vec![0u8; self.block_size].into_boxed_slice();
            buf = self.io_handler.read_block(buf, ind_block_idx).await?;
            self.cur_ind_buf_block_idx = ind_block_idx;
            self.cur_ind_buf = Some(buf);
        }

        let buf = self.cur_ind_buf.as_mut().unwrap();

        let num: &mut u32 = bytemuck::from_bytes_mut(
            &mut buf[offset_in_ind_block * 4..offset_in_ind_block * 4 + 4],
        );

        let mut res = *num;

        if *num == 0 {
            let block = self
                .block_allocator
                .allocate_n_blocks_in_group(self.group_number, 1)
                .await?
                .remove(0);

            *num = block.block_global_idx;
            res = *num;

            self.io_handler
                .write_block(buf.clone(), self.cur_ind_buf_block_idx)
                .await?;

            allocated_blocks.push(block);
        }

        self.cur_block_idx = res;

        Ok(())
    }

    async fn handle_set_indirect_block(
        &mut self,
        // guaranteed to not be 0
        double_ind_block_idx: u32,
        offset_in_double_ind_block: usize,
        offset_in_ind_block: usize,
        allocated_blocks: &mut Vec<AllocatedBlock>,
    ) -> Result<(), HalFsIOErr> {
        if self.cur_double_ind_buf_block_idx != double_ind_block_idx {
            let mut buf = vec![0u8; self.block_size].into_boxed_slice();
            buf = self
                .io_handler
                .read_block(buf, double_ind_block_idx)
                .await?;
            self.cur_double_ind_buf_block_idx = double_ind_block_idx;
            self.cur_double_ind_buf = Some(buf);
        }

        let buf = self.cur_double_ind_buf.as_mut().unwrap();

        let num: &mut u32 = bytemuck::from_bytes_mut(
            &mut buf[offset_in_double_ind_block * 4..offset_in_double_ind_block * 4 + 4],
        );

        let mut ind_block_idx = *num;

        if *num == 0 {
            let block = self
                .block_allocator
                .allocate_n_blocks_in_group(self.group_number, 1)
                .await?
                .remove(0);

            *num = block.block_global_idx;
            ind_block_idx = *num;

            self.io_handler
                .write_block(buf.clone(), self.cur_double_ind_buf_block_idx)
                .await?;

            self.clear_block(block.block_global_idx).await?;
            allocated_blocks.push(block);
        }

        self.handle_set_block(ind_block_idx, offset_in_ind_block, allocated_blocks)
            .await?;

        Ok(())
    }

    async fn handle_set_double_indirect_block(
        &mut self,
        offset_in_triple_ind_block: usize,
        offset_in_double_ind_block: usize,
        offset_in_ind_block: usize,
        allocated_blocks: &mut Vec<AllocatedBlock>,
    ) -> Result<(), HalFsIOErr> {
        let buf = self.cur_triple_ind_buf.as_mut().unwrap();

        let num: &mut u32 = bytemuck::from_bytes_mut(
            &mut buf[offset_in_triple_ind_block * 4..offset_in_triple_ind_block * 4 + 4],
        );

        let mut double_ind_block_idx = *num;

        if *num == 0 {
            let block = self
                .block_allocator
                .allocate_n_blocks_in_group(self.group_number, 1)
                .await?
                .remove(0);

            *num = block.block_global_idx;
            double_ind_block_idx = *num;

            self.io_handler
                .write_block(buf.clone(), self.cur_triple_ind_buf_block_idx)
                .await?;

            self.clear_block(block.block_global_idx).await?;
            allocated_blocks.push(block);
        }

        self.handle_set_indirect_block(
            double_ind_block_idx,
            offset_in_double_ind_block,
            offset_in_ind_block,
            allocated_blocks,
        )
        .await?;

        Ok(())
    }

    async fn handle_block_in_blocks_array(
        &mut self,
        idx: usize,
        allocated_blocks: &mut Vec<AllocatedBlock>,
    ) -> Result<(), HalFsIOErr> {
        if self.blocks[idx] == 0 {
            let block = self
                .block_allocator
                .allocate_n_blocks_in_group(self.group_number, 1)
                .await?
                .remove(0);

            self.blocks[idx] = block.block_global_idx as u32;

            self.clear_block(self.blocks[idx]).await?;

            allocated_blocks.push(block);
        }

        Ok(())
    }

    async fn clear_block(&self, block_idx: u32) -> Result<(), HalFsIOErr> {
        let mut buf = vec![0u8; self.block_size].into_boxed_slice();
        buf.fill(0);
        Ok(self.io_handler.write_block(buf.clone(), block_idx).await?)
    }

    /// allocate a block for the current location
    pub async fn set(&mut self) -> Result<BlockIterSetRes, HalFsIOErr> {
        let blocks_per_group = self.block_size / 4;
        let mut allocated_blocks = vec![];

        if self.cur_idx < INODE_BLOCK_LIMIT as usize {
            if self.blocks[self.cur_idx] == 0 {
                let block = self
                    .block_allocator
                    .allocate_n_blocks_in_group(self.group_number, 1)
                    .await?
                    .remove(0);

                self.blocks[self.cur_idx] = block.block_global_idx as u32;
                allocated_blocks.push(block);
            }

            self.cur_block_idx = self.blocks[self.cur_idx];
        } else if self.cur_idx < INODE_IND_BLOCK_LIMIT as usize {
            self.handle_block_in_blocks_array(INODE_BLOCK_LIMIT as usize, &mut allocated_blocks)
                .await?;

            if self.cur_ind_buf.is_none() {
                let mut buf = vec![0u8; self.block_size].into_boxed_slice();
                buf = self
                    .io_handler
                    .read_block(buf, self.blocks[INODE_BLOCK_LIMIT as usize])
                    .await?;
                self.cur_ind_buf = Some(buf);
                self.cur_ind_buf_block_idx = self.blocks[INODE_BLOCK_LIMIT as usize];
            }

            let ind_block_idx = self.blocks[INODE_BLOCK_LIMIT as usize];
            let offset_in_ind_block = self.cur_idx - INODE_BLOCK_LIMIT as usize;

            self.handle_set_block(ind_block_idx, offset_in_ind_block, &mut allocated_blocks)
                .await?;
        } else if self.cur_idx < INODE_DOUBLE_IND_BLOCK_LIMIT as usize {
            self.handle_block_in_blocks_array(
                INODE_BLOCK_LIMIT as usize + 1,
                &mut allocated_blocks,
            )
            .await?;

            if self.cur_double_ind_buf.is_none() {
                let mut buf = vec![0u8; self.block_size].into_boxed_slice();
                buf = self
                    .io_handler
                    .read_block(buf, self.blocks[INODE_BLOCK_LIMIT as usize + 1])
                    .await?;
                self.cur_double_ind_buf = Some(buf);
                self.cur_double_ind_buf_block_idx = self.blocks[INODE_BLOCK_LIMIT as usize + 1];
            }

            let offset_in_double_ind_block =
                (self.cur_idx - INODE_IND_BLOCK_LIMIT as usize) / blocks_per_group;
            let offset_in_ind_block =
                (self.cur_idx - INODE_IND_BLOCK_LIMIT as usize) % blocks_per_group;

            self.handle_set_indirect_block(
                self.blocks[INODE_BLOCK_LIMIT as usize + 1],
                offset_in_double_ind_block,
                offset_in_ind_block,
                &mut allocated_blocks,
            )
            .await?;
        } else {
            self.handle_block_in_blocks_array(
                INODE_BLOCK_LIMIT as usize + 2,
                &mut allocated_blocks,
            )
            .await?;

            if self.cur_triple_ind_buf.is_none() {
                let mut buf = vec![0u8; self.block_size].into_boxed_slice();
                buf = self
                    .io_handler
                    .read_block(buf, self.blocks[INODE_BLOCK_LIMIT as usize + 2])
                    .await?;
                self.cur_triple_ind_buf = Some(buf);
                self.cur_triple_ind_buf_block_idx = self.blocks[INODE_BLOCK_LIMIT as usize + 2];
            }

            let offset_in_triple_ind_block = (self.cur_idx - INODE_DOUBLE_IND_BLOCK_LIMIT as usize)
                / (blocks_per_group * blocks_per_group);
            let offset_in_double_ind_block = (self.cur_idx - INODE_DOUBLE_IND_BLOCK_LIMIT as usize)
                / blocks_per_group
                % blocks_per_group;
            let offset_in_ind_block =
                (self.cur_idx - INODE_DOUBLE_IND_BLOCK_LIMIT as usize) % blocks_per_group;

            self.handle_set_double_indirect_block(
                offset_in_triple_ind_block,
                offset_in_double_ind_block,
                offset_in_ind_block,
                &mut allocated_blocks,
            )
            .await?;
        }

        Ok(BlockIterSetRes {
            allocated_blocks,
            block_idx: self.cur_block_idx,
        })
    }

    pub fn get_blocks_array(&self) -> [u32; 15] {
        self.blocks
    }

    pub fn into_blocks_array(self) -> [u32; 15] {
        self.blocks
    }
}

pub struct BlockIterElement {
    pub buf: Box<[u8]>,
    pub is_terminated: bool,
    /// if the array is not terminated it will contain the block index of the block, else the value
    /// is undefined
    pub block_idx: u32,
}

pub struct BlockIterSetRes {
    /// if it is empty it means that there was lba originally
    pub allocated_blocks: Vec<AllocatedBlock>,
    /// the address of the block at this index
    pub block_idx: u32,
}
