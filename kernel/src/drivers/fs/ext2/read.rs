use alloc::boxed::Box;
use dvida_serialize::DvDeserialize;

use crate::{
    drivers::fs::ext2::{BLOCK_SIZE, Inode, InodePlus, structs::Ext2Fs},
    hal::fs::{HalFsIOErr, HalIOCtx},
    time,
};

#[derive(Debug)]
pub struct Progress {
    pub block_idx: u32,
    pub offset: u32,
    pub bytes_written: usize,
}

pub const INODE_BLOCK_LIMIT: u32 = 12;
pub const IND_BLOCK_ADDR_COUNT: u32 = BLOCK_SIZE / 4;
pub const INODE_IND_BLOCK_LIMIT: u32 = INODE_BLOCK_LIMIT + IND_BLOCK_ADDR_COUNT;
pub const INODE_DOUBLE_IND_BLOCK_LIMIT: u32 =
    INODE_IND_BLOCK_LIMIT + IND_BLOCK_ADDR_COUNT * IND_BLOCK_ADDR_COUNT;
pub const INODE_TRIPLE_IND_BLOCK_LIMIT: u32 = INODE_DOUBLE_IND_BLOCK_LIMIT
    + IND_BLOCK_ADDR_COUNT * IND_BLOCK_ADDR_COUNT * IND_BLOCK_ADDR_COUNT;
pub const ADDR_PER_BLOCK: u32 = BLOCK_SIZE / 4;

impl Ext2Fs {
    // this function has no bound checks so the i_size check has to be done before calling this
    pub async fn get_block_lba(&self, inode: &Inode, mut idx: u32) -> Result<u32, HalFsIOErr> {
        if idx < INODE_BLOCK_LIMIT {
            // after that we use indirect blocks
            return Ok(inode.i_block[idx as usize]);
        }

        let buf = Box::new([0u8; BLOCK_SIZE as usize]);
        if idx < INODE_IND_BLOCK_LIMIT {
            // after that we use double indirect blocks
            idx = idx - INODE_BLOCK_LIMIT;
            self.read_sectors(buf.clone(), inode.i_block[12] as i64)
                .await?;

            return Ok(u32::deserialize(
                dvida_serialize::Endianness::Little,
                &buf[idx as usize * 4..],
            )?
            .0);
        }

        if idx < INODE_DOUBLE_IND_BLOCK_LIMIT {
            idx = idx - INODE_IND_BLOCK_LIMIT;
            let block_idx = idx / ADDR_PER_BLOCK;
            self.read_sectors(buf.clone(), inode.i_block[13] as i64)
                .await?;

            let ind_block_addr = u32::deserialize(
                dvida_serialize::Endianness::Little,
                &buf[block_idx as usize * 4..],
            )?
            .0 as i64;

            self.read_sectors(buf.clone(), ind_block_addr).await?;

            return Ok(u32::deserialize(
                dvida_serialize::Endianness::Little,
                &buf[(idx % ADDR_PER_BLOCK) as usize * 4..],
            )?
            .0);
        }

        if idx < INODE_TRIPLE_IND_BLOCK_LIMIT {
            idx = idx - INODE_DOUBLE_IND_BLOCK_LIMIT;
            let double_ind_block_idx = idx / ADDR_PER_BLOCK / ADDR_PER_BLOCK;
            let ind_block_idx: u32 = (idx % (ADDR_PER_BLOCK * ADDR_PER_BLOCK)) / ADDR_PER_BLOCK;
            let block_idx: u32 = (idx % (ADDR_PER_BLOCK * ADDR_PER_BLOCK)) % ADDR_PER_BLOCK;

            self.read_sectors(buf.clone(), inode.i_block[13] as i64)
                .await?;

            let double_ind_block_addr = u32::deserialize(
                dvida_serialize::Endianness::Little,
                &buf[double_ind_block_idx as usize * 4..],
            )?
            .0 as i64;

            self.read_sectors(buf.clone(), double_ind_block_addr as i64)
                .await?;

            let ind_block_addr = u32::deserialize(
                dvida_serialize::Endianness::Little,
                &buf[ind_block_idx as usize * 4..],
            )?
            .0 as i64;

            self.read_sectors(buf.clone(), ind_block_addr as i64)
                .await?;

            return Ok(u32::deserialize(
                dvida_serialize::Endianness::Little,
                &buf[block_idx as usize * 4..],
            )?
            .0);
        }

        Err(HalFsIOErr::FileTooLarge)
    }

    async fn read_till_next_block(
        &self,
        inode: &Inode,
        target: &mut [u8],
        ctx: &mut HalIOCtx,
        progress: &mut Progress,
    ) -> Result<(), HalFsIOErr> {
        let lba = self.get_block_lba(inode, progress.block_idx as u32).await?;
        let buf = Box::new([0u8; BLOCK_SIZE as usize]);

        self.read_sectors(buf.clone(), lba as i64).await?;

        for i in progress.offset..self.super_block.block_size() {
            if ctx.head as u32 >= inode.i_size {
                return Ok(());
            }

            if progress.bytes_written >= target.len() {
                return Ok(());
            }

            target[progress.bytes_written] = buf[i as usize];
            progress.bytes_written += 1;
            ctx.head += 1;
        }

        progress.block_idx += 1;
        progress.offset = 0;

        Ok(())
    }

    pub async fn read(
        &mut self,
        victim_inode: &mut InodePlus,
        mut buf: Box<[u8]>,
        ctx: &mut HalIOCtx,
    ) -> Result<usize, HalFsIOErr> {
        let inode = &mut victim_inode.inode;

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

        while (ctx.head as u32) < inode.i_size && progress.bytes_written < buf.len() {
            self.read_till_next_block(inode, &mut buf, ctx, &mut progress)
                .await?;
        }

        let time = time::formats::rtc_to_posix(
            &time::Rtc::new()
                .read_datetime()
                .expect("Failed to get time"),
        );
        inode.i_atime = time;

        self.write_inode(victim_inode).await?;

        Ok(progress.bytes_written)
    }
}
