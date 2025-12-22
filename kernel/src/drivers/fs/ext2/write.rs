use alloc::{boxed::Box, vec, vec::Vec};

use crate::{
    crypto::iterators::{Bit, BitIterator},
    drivers::fs::ext2::{
        BLOCK_SIZE, Inode,
        create_file::{AllocatedBlock, BLOCK_SECTOR_SIZE, RESERVED_BOOT_RECORD_OFFSET},
        read::Progress,
        structs::Ext2Fs,
    },
    hal::{
        fs::{HalFsIOErr, HalIOCtx},
        storage::{HalStorageOperationErr, SECTOR_SIZE},
    },
};

impl Ext2Fs {
    pub async fn allocate_n_blocks(
        &self,
        mut remaining_blocks: usize,
    ) -> Result<Vec<AllocatedBlock>, HalStorageOperationErr> {
        let block_group_size = self.block_group_size();
        let mut blocks_allocated = vec![];

        for (group_number, addr) in (RESERVED_BOOT_RECORD_OFFSET..self.len() + 1)
            .step_by(block_group_size as usize)
            .enumerate()
        {
            if remaining_blocks <= 0 {
                break;
            }

            let mut buf = Box::new([0u8; BLOCK_SIZE as usize]);

            self.read_sectors(buf.clone(), addr + 2 * BLOCK_SECTOR_SIZE)
                .await?;

            let bit_iterator = BitIterator::new(buf.as_mut_slice());

            for (idx, bit) in bit_iterator.into_iter().enumerate() {
                if remaining_blocks <= 0 {
                    break;
                }

                if bit == Bit::One {
                    continue;
                }

                let block_addr = ((block_group_size * group_number as i64
                    + RESERVED_BOOT_RECORD_OFFSET
                    + 5
                    + BLOCK_SECTOR_SIZE)
                    + idx as i64) as u32;
                remaining_blocks -= 1;

                blocks_allocated.push(AllocatedBlock {
                    addr: block_addr as i64,
                    block_idx: idx as i64,
                    gr_number: group_number as i64,
                });
            }
        }

        Ok(blocks_allocated)
    }

    pub async fn allocate_n_blocks_in_group(
        &self,
        group_number: i64,
        mut num: usize,
    ) -> Result<Vec<AllocatedBlock>, HalStorageOperationErr> {
        let mut blocks_allocated = vec![];

        let block_group_size =
            self.super_block.s_blocks_per_group as i64 * (BLOCK_SIZE as i64 / SECTOR_SIZE as i64);

        let addr =
            block_group_size * group_number + RESERVED_BOOT_RECORD_OFFSET + 2 * BLOCK_SECTOR_SIZE;

        let mut buf = Box::new([0u8; BLOCK_SIZE as usize]);

        self.read_sectors(buf.clone(), addr).await?;

        let bit_iterator: BitIterator<u8> = BitIterator::new(buf.as_mut_slice());

        for (idx, bit) in bit_iterator.into_iter().enumerate() {
            if num <= 0 {
                break;
            }

            if bit == Bit::One {
                continue;
            }

            let block_addr = ((block_group_size * group_number
                + RESERVED_BOOT_RECORD_OFFSET
                + 5
                + BLOCK_SECTOR_SIZE)
                + idx as i64) as u32;

            num -= 1;
            blocks_allocated.push(AllocatedBlock {
                addr: block_addr as i64,
                block_idx: idx as i64,
                gr_number: group_number,
            });
        }

        if num <= 0 {
            return Ok(blocks_allocated);
        }

        // iterate over the entire filesystem
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

    pub async fn write(
        &mut self,
        inode: &mut Inode,
        buf: Box<[u8]>,
        ctx: &mut HalIOCtx,
    ) -> Result<usize, HalFsIOErr> {
        if inode.is_directory() {
            return Err(HalFsIOErr::IsDirectory);
        }

        let len = if (inode.i_size - (ctx.head as u32)) % (buf.len() as u32) == 0 {
            buf.len()
        } else {
            buf.len() + ctx.head as usize - inode.i_size as usize
        };

        if buf.len() < len {
            return Err(HalFsIOErr::BufTooSmall);
        }

        let mut progress = Progress {
            block_idx: ctx.head as u32 / self.super_block.block_size(),
            offset: ctx.head as u32 % self.super_block.block_size(),
            bytes_written: 0,
        };

        while progress.bytes_written < buf.len() {
            if progress.block_idx >= inode.i_blocks {}

            self.write_till_next_block(inode, buf.clone(), ctx, &mut progress)
                .await?;
        }

        Ok(progress.bytes_written)
    }
}
