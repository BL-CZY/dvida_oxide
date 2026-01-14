use alloc::boxed::Box;
use dvida_serialize::{DvDeserialize, DvSerialize};

use crate::{
    drivers::fs::ext2::{
        BLOCK_SIZE, InodePlus,
        create_file::RESERVED_BOOT_RECORD_OFFSET,
        read::{INODE_BLOCK_LIMIT, INODE_DOUBLE_IND_BLOCK_LIMIT, INODE_IND_BLOCK_LIMIT},
        structs::Ext2Fs,
    },
    hal::{fs::HalFsIOErr, path::Path},
};

impl Ext2Fs {
    pub async fn delete_file(&mut self, path: Path) -> Result<(), HalFsIOErr> {
        let (directory_inode, file_inode) = self.walk_path(&path).await?;
        self.find_entry_by_name_and_delete(
            &path.file_name().ok_or(HalFsIOErr::BadPath)?,
            &directory_inode,
        )
        .await?;

        let Some(mut file_inode) = file_inode else {
            return Err(HalFsIOErr::NoSuchFileOrDirectory);
        };

        file_inode.inode.i_links_count -= 1;

        if file_inode.inode.i_links_count == 0 {
            self.free_inode(&mut file_inode).await?;
        }

        Ok(())
    }

    /// doesn't write changes to the super block
    pub async fn free_block(
        &mut self,
        block_lba: u32,
        cur_bitmap_lba: &mut i64,
        mut buf: Box<[u8]>,
    ) -> Result<Box<[u8]>, HalFsIOErr> {
        let block_group = self
            .group_manager
            .get_group_from_block_idx(block_lba)
            .await?;
        let bitmap_lba = block_group.get_block_bitmap_lba();

        if *cur_bitmap_lba != bitmap_lba {
            buf = self.read_sectors(buf, bitmap_lba).await?;
            *cur_bitmap_lba = bitmap_lba;
        }

        let block_rel_idx = block_lba as usize % self.group_manager.blocks_per_group as usize;

        buf[block_rel_idx / 8] = buf[block_rel_idx / 8] & !(1 << (block_rel_idx % 8));
        self.write_sectors(buf.clone(), bitmap_lba).await?;

        self.block_allocator.add_freed_block(block_lba);

        Ok(buf)
    }

    pub async fn free_indirect_block(
        &mut self,
        block_idx: u32,
        cur_bitmap_lba: &mut i64,
        mut cur_buf: Box<[u8]>,
    ) -> Result<Box<[u8]>, HalFsIOErr> {
        let mut buf: Box<[u8]> = Box::new([0u8; BLOCK_SIZE as usize]);
        buf = self.io_handler.read_block(buf, block_idx).await?;
        for i in (0..BLOCK_SIZE).step_by(4) {
            let idx = u32::deserialize(dvida_serialize::Endianness::Little, &buf[i as usize..])?.0;
            if idx == 0 {
                // remaining pointers are zero; stop iterating so we still free the
                // indirect block itself below
                break;
            }

            cur_buf = self.free_block(idx, cur_bitmap_lba, cur_buf).await?;
        }

        // finally free the indirect block entry itself
        cur_buf = self.free_block(block_idx, cur_bitmap_lba, cur_buf).await?;

        Ok(cur_buf)
    }

    pub async fn free_double_indirect_block(
        &mut self,
        block_idx: u32,
        cur_bitmap_lba: &mut i64,
        mut cur_buf: Box<[u8]>,
    ) -> Result<Box<[u8]>, HalFsIOErr> {
        let mut buf: Box<[u8]> = Box::new([0u8; BLOCK_SIZE as usize]);
        buf = self.io_handler.read_block(buf, block_idx).await?;
        for i in (0..BLOCK_SIZE).step_by(4) {
            let lba = u32::deserialize(dvida_serialize::Endianness::Little, &buf[i as usize..])?.0;
            if lba == 0 {
                break;
            }

            // lba is the address of an indirect block
            cur_buf = self
                .free_indirect_block(block_idx, cur_bitmap_lba, cur_buf)
                .await?;
        }

        // finally free the double-indirect block itself
        cur_buf = self.free_block(block_idx, cur_bitmap_lba, cur_buf).await?;

        Ok(cur_buf)
    }

    pub async fn free_triple_indirect_block(
        &mut self,
        block_idx: u32,
        cur_bitmap_lba: &mut i64,
        mut cur_buf: Box<[u8]>,
    ) -> Result<Box<[u8]>, HalFsIOErr> {
        let mut buf: Box<[u8]> = Box::new([0u8; BLOCK_SIZE as usize]);
        buf = self.io_handler.read_block(buf, block_idx).await?;
        for i in (0..BLOCK_SIZE).step_by(4) {
            let block_idx =
                u32::deserialize(dvida_serialize::Endianness::Little, &buf[i as usize..])?.0;
            if block_idx == 0 {
                break;
            }

            // lba is the address of a double-indirect block
            cur_buf = self
                .free_double_indirect_block(block_idx, cur_bitmap_lba, cur_buf)
                .await?;
        }

        // finally free the triple-indirect block itself
        cur_buf = self.free_block(block_idx, cur_bitmap_lba, cur_buf).await?;

        Ok(cur_buf)
    }

    /// doesn't update the changes in the superblock to the filesystem
    pub async fn free_blocks(&mut self, inode: &mut InodePlus) -> Result<(), HalFsIOErr> {
        let mut cur_buf: Box<[u8]> = Box::new([0u8; BLOCK_SIZE as usize]);
        let mut cur_bitmap_lba = 0;
        for i in 0..INODE_BLOCK_LIMIT as usize {
            if inode.inode.i_block[i] == 0 {
                return Ok(());
            }

            cur_buf = self
                .free_block(inode.inode.i_block[i], &mut cur_bitmap_lba, cur_buf)
                .await?;
        }

        if inode.inode.i_blocks > INODE_BLOCK_LIMIT {
            cur_buf = self
                .free_indirect_block(
                    inode.inode.i_block[INODE_BLOCK_LIMIT as usize],
                    &mut cur_bitmap_lba,
                    cur_buf,
                )
                .await?;
        }
        if inode.inode.i_blocks > INODE_IND_BLOCK_LIMIT {
            cur_buf = self
                .free_double_indirect_block(
                    inode.inode.i_block[INODE_BLOCK_LIMIT as usize + 1],
                    &mut cur_bitmap_lba,
                    cur_buf,
                )
                .await?;
        }
        if inode.inode.i_blocks > INODE_DOUBLE_IND_BLOCK_LIMIT {
            cur_buf = self
                .free_triple_indirect_block(
                    inode.inode.i_block[INODE_BLOCK_LIMIT as usize + 2],
                    &mut cur_bitmap_lba,
                    cur_buf,
                )
                .await?;
        }

        self.super_block.s_free_blocks_count +=
            inode.inode.i_blocks / self.super_block.block_size();

        Ok(())
    }

    pub async fn free_inode(&mut self, inode: &mut InodePlus) -> Result<(), HalFsIOErr> {
        self.free_blocks(inode).await?;

        let time = crate::time::formats::rtc_to_posix(
            &crate::time::Rtc::new()
                .read_datetime()
                .expect("Failed to get time"),
        );

        inode.inode.i_dtime = time;
        inode.inode.i_blocks = 0;

        self.write_inode(inode).await?;

        let inode_bitmap_lba = self
            .get_group(inode.group_number as i64)
            .await?
            .get_inode_bitmap_lba();

        let mut buf: Box<[u8]> = Box::new([0u8; BLOCK_SIZE as usize]);
        buf = self.read_sectors(buf, inode_bitmap_lba).await?;

        buf[inode.relative_idx as usize / 8] =
            buf[inode.relative_idx as usize / 8] & !(1 << (inode.relative_idx % 8));

        self.write_sectors(buf.clone(), inode_bitmap_lba).await?;

        self.super_block.s_free_inodes_count += 1;

        buf.fill(0);

        let super_block_bytes = bytemuck::bytes_of(&self.super_block);
        for i in 0..super_block_bytes.len() {
            buf[i] = super_block_bytes[i];
        }

        self.write_sectors(buf, RESERVED_BOOT_RECORD_OFFSET).await?;
        self.block_allocator.write_freed_blocks().await?;

        Ok(())
    }
}
