use crate::log;
use crate::time;
use alloc::{boxed::Box, vec::Vec};

use crate::{
    drivers::fs::ext2::{
        BLOCK_SIZE, Inode, InodePlus, create_file::AllocatedBlock, read::Progress, structs::Ext2Fs,
    },
    hal::fs::{HalFsIOErr, HalIOCtx},
};

impl Ext2Fs {
    pub async fn allocate_n_blocks(
        &mut self,
        exclude_group_idx: i64,
        remaining_blocks: usize,
    ) -> Result<Vec<AllocatedBlock>, HalFsIOErr> {
        self.block_allocator
            .allocate_n_blocks(exclude_group_idx, remaining_blocks)
            .await
    }

    pub async fn allocate_n_blocks_in_group(
        &mut self,
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
        input: &[u8],
        ctx: &mut HalIOCtx,
        block_idx: u32,
        progress: &mut Progress,
    ) -> Result<(), HalFsIOErr> {
        log!("Prepared to write input for block {block_idx}");
        let mut buf: Box<[u8]> = Box::new([0u8; BLOCK_SIZE as usize]);

        // if we are not at the start of a block we need to make sure the existing data doesn't
        // get overwritten
        if progress.offset != 0 {
            buf = self.io_handler.read_block(buf, block_idx).await?;
        }

        for i in progress.offset..self.super_block.block_size() {
            if ctx.head as u32 >= inode.i_size {
                inode.i_size += 1;
            }

            if progress.bytes_written >= input.len() {
                break;
            }

            buf[i as usize] = input[progress.bytes_written];
            progress.bytes_written += 1;
            ctx.head += 1;
        }

        self.io_handler.write_block(buf.clone(), block_idx).await?;
        progress.block_idx += 1;
        progress.offset = 0;

        Ok(())
    }

    pub async fn write(
        &mut self,
        victim_inode: &mut InodePlus,
        buf: &[u8],
        ctx: &mut HalIOCtx,
    ) -> Result<usize, HalFsIOErr> {
        log!("write: input: {:?}", buf);
        let inode = &mut victim_inode.inode;

        if inode.is_directory() {
            return Err(HalFsIOErr::IsDirectory);
        }

        let mut progress = Progress {
            block_idx: ctx.head as u32 / self.super_block.block_size(),
            offset: ctx.head as u32 % self.super_block.block_size(),
            bytes_written: 0,
        };

        let mut blocks_allocated_count = 0;

        let mut iterator = self.create_block_iterator(inode, victim_inode.group_number.into());
        iterator.skip(progress.block_idx as usize);
        while progress.bytes_written < buf.len() {
            let res = iterator.next_set().await?;
            blocks_allocated_count += res.allocated_blocks.len();
            self.write_till_next_block(inode, &buf, ctx, res.block_idx, &mut progress)
                .await?;
        }

        let time = time::formats::rtc_to_posix(
            &time::Rtc::new()
                .read_datetime()
                .expect("Failed to get time"),
        );
        inode.i_mtime = time;
        inode.i_block = iterator.into_blocks_array();
        inode.i_blocks += blocks_allocated_count as u32 * self.super_block.block_size() as u32;

        self.write_inode(victim_inode).await?;
        let buf = self.get_buffer();
        self.block_allocator
            .write_newly_allocated_blocks(buf)
            .await?;

        Ok(progress.bytes_written)
    }
}
