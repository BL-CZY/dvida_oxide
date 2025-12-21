use alloc::boxed::Box;

use crate::{
    drivers::fs::ext2::{BLOCK_SIZE, Inode, read::Progress, structs::Ext2Fs},
    hal::fs::{HalFsIOErr, HalIOCtx},
};

impl Ext2Fs {
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
