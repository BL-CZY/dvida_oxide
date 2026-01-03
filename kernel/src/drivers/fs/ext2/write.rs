use crate::{hal::storage::SECTOR_SIZE, time};
use alloc::{boxed::Box, vec, vec::Vec};
use dvida_serialize::{DvDeserialize, DvSerialize};

use crate::{
    drivers::fs::ext2::{
        BLOCK_SIZE, Inode, InodePlus,
        create_file::AllocatedBlock,
        read::{INODE_BLOCK_LIMIT, INODE_DOUBLE_IND_BLOCK_LIMIT, INODE_IND_BLOCK_LIMIT, Progress},
        structs::Ext2Fs,
    },
    hal::fs::{HalFsIOErr, HalIOCtx},
};

impl Ext2Fs {
    pub async fn allocate_n_blocks(
        &self,
        exclude_group_idx: i64,
        remaining_blocks: usize,
    ) -> Result<Vec<AllocatedBlock>, HalFsIOErr> {
        self.block_allocator
            .allocate_n_blocks(exclude_group_idx, remaining_blocks)
            .await
    }

    pub async fn allocate_n_blocks_in_group(
        &self,
        group_number: i64,
        num: usize,
    ) -> Result<Vec<AllocatedBlock>, HalFsIOErr> {
        self.block_allocator
            .allocate_n_blocks_in_group(group_number, num)
            .await
    }

    async fn write_till_next_block(
        &mut self,
        inode: &mut Inode,
        input: &Box<[u8]>,
        ctx: &mut HalIOCtx,
        progress: &mut Progress,
    ) -> Result<(), HalFsIOErr> {
        let mut buf: Box<[u8]> = Box::new([0u8; BLOCK_SIZE as usize]);
        let lba = self.get_block_lba(inode, progress.block_idx).await? as i64;

        // if we are not at the start of a block we need to make sure the existing data doesn't
        // get overwritten
        if progress.offset != 0 {
            buf = self.read_sectors(buf, lba).await?;
        }

        for i in progress.offset..self.super_block.block_size() {
            if ctx.head as u32 >= inode.i_size {
                inode.i_size += 1;
            }

            if progress.bytes_written >= input.len() {
                return Ok(());
            }

            buf[i as usize] = input[progress.bytes_written];
            progress.bytes_written += 1;
            ctx.head += 1;
        }

        self.write_sectors(buf.clone(), lba).await?;
        progress.block_idx += 1;
        progress.offset = 0;

        Ok(())
    }

    pub async fn handle_indirect_block(
        &mut self,
        addr: u32,
        inode: &mut Inode,
        cur_ind_block_lba: &mut i64,
        mut cur_ind_block_buf: Box<[u8]>,
        group_number: i64,
        newly_allocated_blocks: &mut Vec<AllocatedBlock>,
        block: &AllocatedBlock,
        num_blocks: &mut u32,
    ) -> Result<Box<[u8]>, HalFsIOErr> {
        if addr == 0 {
            let ind_block = self
                .allocate_n_blocks_in_group(group_number, 1)
                .await?
                .remove(0);
            *cur_ind_block_lba = ind_block.addr;
            cur_ind_block_buf.fill(0);
            inode.i_block[INODE_BLOCK_LIMIT as usize] = ind_block.addr as u32;

            newly_allocated_blocks.push(ind_block);

            block.addr.serialize(
                dvida_serialize::Endianness::Little,
                &mut cur_ind_block_buf[(*num_blocks - INODE_BLOCK_LIMIT) as usize * 4..],
            )?;

            return Ok(cur_ind_block_buf);
        }

        if *cur_ind_block_lba as u32 != inode.i_block[INODE_BLOCK_LIMIT as usize] {
            if *cur_ind_block_lba != 0 {
                self.write_sectors(cur_ind_block_buf.clone(), *cur_ind_block_lba)
                    .await?;
            }

            *cur_ind_block_lba = inode.i_block[INODE_BLOCK_LIMIT as usize] as i64;
            cur_ind_block_buf = self
                .read_sectors(cur_ind_block_buf, *cur_ind_block_lba)
                .await?;

            block.addr.serialize(
                dvida_serialize::Endianness::Little,
                &mut cur_ind_block_buf[(*num_blocks - INODE_BLOCK_LIMIT) as usize * 4..],
            )?;
        }

        *num_blocks += 1;

        Ok(cur_ind_block_buf)
    }

    pub async fn handle_double_indirect_block(
        &mut self,
        addr: u32,
        group_number: i64,
        inode: &mut Inode,
        newly_allocated_blocks: &mut Vec<AllocatedBlock>,
        double_ind_block_buf: &mut Option<Box<[u8; 1024]>>,
        cur_ind_block_lba: &mut i64,
        mut cur_ind_block_buf: Box<[u8]>,
        block: &AllocatedBlock,
        num_blocks: &mut u32,
    ) -> Result<Box<[u8]>, HalFsIOErr> {
        if addr == 0 {
            let double_ind_block = self
                .allocate_n_blocks_in_group(group_number, 1)
                .await?
                .remove(0);

            inode.i_block[INODE_BLOCK_LIMIT as usize + 1] = double_ind_block.addr as u32;
            newly_allocated_blocks.push(double_ind_block);
            *double_ind_block_buf = Some(Box::new([0u8; BLOCK_SIZE as usize]));
        }

        let temp_buf = match double_ind_block_buf {
            Some(buf) => buf.clone(),
            None => {
                let mut buf: Box<[u8]> = Box::new([0u8; BLOCK_SIZE as usize]);
                buf = self
                    .read_sectors(buf, inode.i_block[INODE_BLOCK_LIMIT as usize + 1] as i64)
                    .await?;
                buf
            }
        };

        let addr = u32::deserialize(
            dvida_serialize::Endianness::Little,
            &temp_buf[4 * ((*num_blocks - INODE_IND_BLOCK_LIMIT) / (BLOCK_SIZE / 4)) as usize..],
        )?
        .0;

        cur_ind_block_buf = self
            .handle_indirect_block(
                addr,
                inode,
                cur_ind_block_lba,
                cur_ind_block_buf,
                group_number,
                newly_allocated_blocks,
                block,
                num_blocks,
            )
            .await?;

        Ok(cur_ind_block_buf)
    }

    /// doesn't write inodes and i_size
    /// TODO: fix it later
    pub async fn expand_inode(
        &mut self,
        inode: &mut Inode,
        group_number: i64,
        len: usize,
    ) -> Result<(), HalFsIOErr> {
        let block_count = len / BLOCK_SIZE as usize;
        let blocks = self
            .allocate_n_blocks_in_group(group_number, block_count)
            .await?;

        let mut num_blocks =
            (inode.i_size + self.super_block.block_size() - 1) / self.super_block.block_size();

        let mut cur_ind_block_lba = 0;
        let mut cur_ind_block_buf: Box<[u8]> = Box::new([0u8; BLOCK_SIZE as usize]);

        let mut double_ind_block_buf = None;
        let mut triple_ind_block_buf = None;

        let mut newly_allocated_blocks: Vec<AllocatedBlock> = vec![];

        for block in blocks.iter() {
            if num_blocks < INODE_BLOCK_LIMIT {
                inode.i_block[num_blocks as usize] = block.addr as u32;
                num_blocks += 1;
            } else if num_blocks < INODE_IND_BLOCK_LIMIT {
                self.handle_indirect_block(
                    inode.i_block[INODE_BLOCK_LIMIT as usize],
                    inode,
                    &mut cur_ind_block_lba,
                    cur_ind_block_buf.clone(),
                    group_number,
                    &mut newly_allocated_blocks,
                    block,
                    &mut num_blocks,
                )
                .await?;
            } else if num_blocks < INODE_DOUBLE_IND_BLOCK_LIMIT {
                cur_ind_block_buf = self
                    .handle_double_indirect_block(
                        inode.i_block[INODE_BLOCK_LIMIT as usize + 1],
                        group_number,
                        inode,
                        &mut newly_allocated_blocks,
                        &mut double_ind_block_buf,
                        &mut cur_ind_block_lba,
                        cur_ind_block_buf,
                        block,
                        &mut num_blocks,
                    )
                    .await?;
            } else {
                if inode.i_block[INODE_BLOCK_LIMIT as usize + 2] == 0 {
                    let triple_ind_block = self
                        .allocate_n_blocks_in_group(group_number, 1)
                        .await?
                        .remove(0);

                    inode.i_block[INODE_BLOCK_LIMIT as usize + 2] = triple_ind_block.addr as u32;
                    newly_allocated_blocks.push(triple_ind_block);
                    triple_ind_block_buf = Some(Box::new([0u8; BLOCK_SIZE as usize]));
                }

                let temp_buf = match triple_ind_block_buf {
                    Some(ref buf) => buf.clone(),
                    None => {
                        let mut buf: Box<[u8]> = Box::new([0u8; BLOCK_SIZE as usize]);
                        // should read the triple-indirect block pointer at index +2
                        buf = self
                            .read_sectors(buf, inode.i_block[INODE_BLOCK_LIMIT as usize + 2] as i64)
                            .await?;
                        buf
                    }
                };

                let addr = u32::deserialize(
                    dvida_serialize::Endianness::Little,
                    &temp_buf[4
                        * ((num_blocks - INODE_DOUBLE_IND_BLOCK_LIMIT)
                            / ((BLOCK_SIZE / 4) * (BLOCK_SIZE / 4)))
                            as usize..],
                )?
                .0;

                self.handle_double_indirect_block(
                    addr,
                    group_number,
                    inode,
                    &mut newly_allocated_blocks,
                    &mut double_ind_block_buf,
                    &mut cur_ind_block_lba,
                    cur_ind_block_buf.clone(),
                    block,
                    &mut num_blocks,
                )
                .await?;
            }
        }

        inode.i_blocks += (blocks.len() + newly_allocated_blocks.len()) as u32
            * self.super_block.block_size()
            / SECTOR_SIZE as u32;

        self.write_sectors(cur_ind_block_buf.clone(), cur_ind_block_lba)
            .await?;

        // repurpose the buffer as the buffer for the write_newly_allocated_blocks function;
        let buf = cur_ind_block_buf;
        self.write_newly_allocated_blocks(buf.clone(), &blocks)
            .await?;
        self.write_newly_allocated_blocks(buf, &newly_allocated_blocks)
            .await?;

        Ok(())
    }

    pub async fn write(
        &mut self,
        victim_inode: &mut InodePlus,
        buf: Box<[u8]>,
        ctx: &mut HalIOCtx,
    ) -> Result<usize, HalFsIOErr> {
        let inode = &mut victim_inode.inode;

        if inode.is_directory() {
            return Err(HalFsIOErr::IsDirectory);
        }

        let aligned_up_size = ((inode.i_size + self.super_block.block_size() - 1)
            & !(self.super_block.block_size() - 1)) as usize;

        if ctx.head + buf.len() >= aligned_up_size {
            self.expand_inode(
                inode,
                victim_inode.group_number as i64,
                ctx.head + buf.len() - aligned_up_size,
            )
            .await?;
        }

        let mut progress = Progress {
            block_idx: ctx.head as u32 / self.super_block.block_size(),
            offset: ctx.head as u32 % self.super_block.block_size(),
            bytes_written: 0,
        };

        while progress.bytes_written < buf.len() {
            self.write_till_next_block(inode, &buf, ctx, &mut progress)
                .await?;
        }

        let time = time::formats::rtc_to_posix(
            &time::Rtc::new()
                .read_datetime()
                .expect("Failed to get time"),
        );
        inode.i_atime = time;
        inode.i_ctime = time;

        self.write_inode(victim_inode).await?;

        Ok(progress.bytes_written)
    }
}
