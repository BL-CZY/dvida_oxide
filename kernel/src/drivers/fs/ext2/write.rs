use crate::time;
use alloc::{boxed::Box, vec, vec::Vec};
use dvida_serialize::{DvDeserialize, DvSerialize};

use crate::{
    crypto::iterators::{Bit, BitIterator},
    drivers::fs::ext2::{
        BLOCK_SIZE, Inode, InodePlus,
        create_file::AllocatedBlock,
        read::{INODE_BLOCK_LIMIT, INODE_DOUBLE_IND_BLOCK_LIMIT, INODE_IND_BLOCK_LIMIT, Progress},
        structs::Ext2Fs,
    },
    hal::{
        fs::{HalFsIOErr, HalIOCtx},
        storage::HalStorageOperationErr,
    },
};

impl Ext2Fs {
    pub async fn allocate_n_blocks(
        &self,
        mut remaining_blocks: usize,
    ) -> Result<Vec<AllocatedBlock>, HalStorageOperationErr> {
        let mut blocks_allocated = vec![];

        // iterate over block groups
        for group_number in 0..(self.super_block.s_blocks_per_group as i64) {
            if remaining_blocks == 0 {
                break;
            }

            let group = self.get_group(group_number as i64);
            let mut buf = Box::new([0u8; BLOCK_SIZE as usize]);

            // read block bitmap for the group
            self.read_sectors(buf.clone(), group.get_block_bitmap_lba())
                .await?;

            let bit_iterator = BitIterator::new(buf.as_mut_slice());

            // compute index of first data block within the group (in bitmap block indices)
            let first_data_block_lba = group.get_data_blocks_start_lba();
            let first_data_block_idx =
                ((first_data_block_lba - group.get_group_lba()) / group.sectors_per_block) as usize;

            for (idx, bit) in bit_iterator.into_iter().enumerate() {
                if remaining_blocks == 0 {
                    break;
                }

                // skip metadata/reserved blocks before the data blocks start
                if idx < first_data_block_idx {
                    continue;
                }

                if bit == Bit::One {
                    continue;
                }

                // map bitmap index to data block LBA
                let block_lba = group.get_data_blocks_start_lba()
                    + ((idx - first_data_block_idx) as i64) * group.sectors_per_block;

                remaining_blocks -= 1;

                blocks_allocated.push(AllocatedBlock {
                    addr: block_lba,
                    block_idx: idx as i64,
                    gr_number: group_number as i64,
                });
            }
        }

        if remaining_blocks > 0 {
            return Err(HalStorageOperationErr::NoEnoughSpace);
        }

        Ok(blocks_allocated)
    }

    pub async fn allocate_n_blocks_in_group(
        &self,
        group_number: i64,
        mut num: usize,
    ) -> Result<Vec<AllocatedBlock>, HalStorageOperationErr> {
        let mut blocks_allocated = vec![];
        let group = self.get_group(group_number);
        let mut buf = Box::new([0u8; BLOCK_SIZE as usize]);

        // read the block bitmap for the group
        self.read_sectors(buf.clone(), group.get_block_bitmap_lba())
            .await?;
        let bit_iterator: BitIterator<u8> = BitIterator::new(buf.as_mut_slice());

        // index of first data block within the group
        let first_data_block_lba = group.get_data_blocks_start_lba();
        let first_data_block_idx =
            ((first_data_block_lba - group.get_group_lba()) / group.sectors_per_block) as usize;

        for (idx, bit) in bit_iterator.into_iter().enumerate() {
            if num == 0 {
                break;
            }

            if idx < first_data_block_idx {
                continue;
            }

            if bit == Bit::One {
                continue;
            }

            let block_lba = group.get_data_blocks_start_lba()
                + ((idx - first_data_block_idx) as i64) * group.sectors_per_block;

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
        blocks_allocated.extend(self.allocate_n_blocks(num).await?.into_iter());

        Ok(blocks_allocated)
    }

    async fn write_till_next_block(
        &mut self,
        inode: &mut Inode,
        input: Box<[u8]>,
        ctx: &mut HalIOCtx,
        progress: &mut Progress,
    ) -> Result<(), HalFsIOErr> {
        let mut buf = Box::new([0u8; BLOCK_SIZE as usize]);
        let lba = self.get_block_lba(inode, progress.block_idx).await? as i64;

        // if we are not at the start of a block we need to make sure the existing data doesn't
        // get overwritten
        if progress.offset != 0 {
            self.read_sectors(buf.clone(), lba).await?;
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
        mut cur_ind_block_buf: Box<[u8; BLOCK_SIZE as usize]>,
        group_number: i64,
        newly_allocated_blocks: &mut Vec<AllocatedBlock>,
        block: &AllocatedBlock,
    ) -> Result<(), HalFsIOErr> {
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
                &mut cur_ind_block_buf[(inode.i_blocks - INODE_BLOCK_LIMIT) as usize * 4..],
            )?;

            return Ok(());
        }

        if *cur_ind_block_lba as u32 != inode.i_block[INODE_BLOCK_LIMIT as usize] {
            if *cur_ind_block_lba != 0 {
                self.write_sectors(cur_ind_block_buf.clone(), *cur_ind_block_lba)
                    .await?;
            }

            *cur_ind_block_lba = inode.i_block[INODE_BLOCK_LIMIT as usize] as i64;
            self.read_sectors(cur_ind_block_buf.clone(), *cur_ind_block_lba)
                .await?;

            block.addr.serialize(
                dvida_serialize::Endianness::Little,
                &mut cur_ind_block_buf[(inode.i_blocks - INODE_BLOCK_LIMIT) as usize * 4..],
            )?;
        }

        Ok(())
    }

    pub async fn handle_double_indirect_block(
        &mut self,
        addr: u32,
        group_number: i64,
        inode: &mut Inode,
        newly_allocated_blocks: &mut Vec<AllocatedBlock>,
        double_ind_block_buf: &mut Option<Box<[u8; 1024]>>,
        cur_ind_block_lba: &mut i64,
        cur_ind_block_buf: Box<[u8; BLOCK_SIZE as usize]>,
        block: &AllocatedBlock,
    ) -> Result<(), HalFsIOErr> {
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
                let buf = Box::new([0u8; BLOCK_SIZE as usize]);
                self.read_sectors(
                    buf.clone(),
                    inode.i_block[INODE_BLOCK_LIMIT as usize + 1] as i64,
                )
                .await?;
                buf
            }
        };

        let addr = u32::deserialize(
            dvida_serialize::Endianness::Little,
            &temp_buf[((inode.i_blocks - INODE_IND_BLOCK_LIMIT) / (BLOCK_SIZE / 4)) as usize..],
        )?
        .0;

        self.handle_indirect_block(
            addr,
            inode,
            cur_ind_block_lba,
            cur_ind_block_buf.clone(),
            group_number,
            newly_allocated_blocks,
            block,
        )
        .await?;

        Ok(())
    }

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

        let mut cur_ind_block_lba = 0;
        let cur_ind_block_buf = Box::new([0u8; BLOCK_SIZE as usize]);

        let mut double_ind_block_buf = None;
        let mut triple_ind_block_buf = None;

        let mut newly_allocated_blocks: Vec<AllocatedBlock> = vec![];

        for block in blocks.iter() {
            if inode.i_blocks < INODE_BLOCK_LIMIT {
                inode.i_block[inode.i_blocks as usize] = block.addr as u32;
                inode.i_blocks += 1;
            } else if inode.i_blocks < INODE_IND_BLOCK_LIMIT {
                self.handle_indirect_block(
                    inode.i_block[INODE_BLOCK_LIMIT as usize],
                    inode,
                    &mut cur_ind_block_lba,
                    cur_ind_block_buf.clone(),
                    group_number,
                    &mut newly_allocated_blocks,
                    block,
                )
                .await?;
            } else if inode.i_blocks < INODE_DOUBLE_IND_BLOCK_LIMIT {
                self.handle_double_indirect_block(
                    inode.i_block[INODE_BLOCK_LIMIT as usize + 1],
                    group_number,
                    inode,
                    &mut newly_allocated_blocks,
                    &mut double_ind_block_buf,
                    &mut cur_ind_block_lba,
                    cur_ind_block_buf.clone(),
                    block,
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
                        let buf = Box::new([0u8; BLOCK_SIZE as usize]);
                        // should read the triple-indirect block pointer at index +2
                        self.read_sectors(
                            buf.clone(),
                            inode.i_block[INODE_BLOCK_LIMIT as usize + 2] as i64,
                        )
                        .await?;
                        buf
                    }
                };

                let addr = u32::deserialize(
                    dvida_serialize::Endianness::Little,
                    &temp_buf[((inode.i_blocks - INODE_DOUBLE_IND_BLOCK_LIMIT)
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
                )
                .await?;
            }
        }

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

        if ctx.head + buf.len() >= inode.i_size as usize {
            self.expand_inode(
                inode,
                victim_inode.group_number as i64,
                ctx.head + buf.len(),
            )
            .await?;
        }

        let mut progress = Progress {
            block_idx: ctx.head as u32 / self.super_block.block_size(),
            offset: ctx.head as u32 % self.super_block.block_size(),
            bytes_written: 0,
        };

        while progress.bytes_written < buf.len() {
            self.write_till_next_block(inode, buf.clone(), ctx, &mut progress)
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
