use alloc::boxed::Box;
use dvida_serialize::{DvDeserialize, DvSerialize};

use crate::{
    drivers::fs::ext2::{
        BLOCK_SIZE, InodePlus,
        create_file::RESERVED_BOOT_RECORD_OFFSET,
        read::{INODE_BLOCK_LIMIT, INODE_DOUBLE_IND_BLOCK_LIMIT, INODE_IND_BLOCK_LIMIT},
        structs::Ext2Fs,
    },
    hal::{fs::HalFsIOErr, path::Path, storage::SECTOR_SIZE},
};

impl Ext2Fs {
    pub async fn delete_file(&mut self, path: Path) -> Result<(), HalFsIOErr> {
        let (directory_inode, file_inode) = self.walk_path(&path).await?;
        self.find_entry_by_name_and_delete(
            &path.file_name().ok_or(HalFsIOErr::BadPath)?,
            &directory_inode.inode,
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
        block_lba: i64,
        cur_bitmap_lba: &mut i64,
        buf: &mut Box<[u8; BLOCK_SIZE as usize]>,
    ) -> Result<(), HalFsIOErr> {
        let block_group = self.get_group_from_lba(block_lba);
        let bitmap_lba = block_group.get_block_bitmap_lba();

        if *cur_bitmap_lba != bitmap_lba {
            self.read_sectors(buf.clone(), bitmap_lba).await?;
            *cur_bitmap_lba = bitmap_lba;
        }

        let data_blocks_start = block_group.get_data_blocks_start_lba();
        let block_idx =
            ((block_lba - data_blocks_start) / (BLOCK_SIZE as i64 / SECTOR_SIZE as i64)) as usize;

        buf[block_idx / 8] = buf[block_idx / 8] & !(1 << (block_idx % 8));
        self.write_sectors(buf.clone(), bitmap_lba).await?;

        Ok(())
    }

    pub async fn free_indirect_block(
        &mut self,
        block_lba: i64,
        cur_bitmap_lba: &mut i64,
        cur_buf: &mut Box<[u8; BLOCK_SIZE as usize]>,
    ) -> Result<(), HalFsIOErr> {
        let buf = Box::new([0u8; BLOCK_SIZE as usize]);
        self.read_sectors(buf.clone(), block_lba).await?;
        for i in (0..BLOCK_SIZE).step_by(4) {
            let lba = u32::deserialize(dvida_serialize::Endianness::Little, &buf[i as usize..])?.0;
            if lba == 0 {
                // remaining pointers are zero; stop iterating so we still free the
                // indirect block itself below
                break;
            }

            self.free_block(lba as i64, cur_bitmap_lba, cur_buf).await?;
        }

        // finally free the indirect block entry itself
        self.free_block(block_lba, cur_bitmap_lba, cur_buf).await?;

        Ok(())
    }

    pub async fn free_double_indirect_block(
        &mut self,
        block_lba: i64,
        cur_bitmap_lba: &mut i64,
        cur_buf: &mut Box<[u8; BLOCK_SIZE as usize]>,
    ) -> Result<(), HalFsIOErr> {
        let buf = Box::new([0u8; BLOCK_SIZE as usize]);
        self.read_sectors(buf.clone(), block_lba).await?;
        for i in (0..BLOCK_SIZE).step_by(4) {
            let lba = u32::deserialize(dvida_serialize::Endianness::Little, &buf[i as usize..])?.0;
            if lba == 0 {
                break;
            }

            // lba is the address of an indirect block
            self.free_indirect_block(lba as i64, cur_bitmap_lba, cur_buf)
                .await?;
        }

        // finally free the double-indirect block itself
        self.free_block(block_lba, cur_bitmap_lba, cur_buf).await?;

        Ok(())
    }

    pub async fn free_triple_indirect_block(
        &mut self,
        block_lba: i64,
        cur_bitmap_lba: &mut i64,
        cur_buf: &mut Box<[u8; BLOCK_SIZE as usize]>,
    ) -> Result<(), HalFsIOErr> {
        let buf = Box::new([0u8; BLOCK_SIZE as usize]);
        self.read_sectors(buf.clone(), block_lba).await?;
        for i in (0..BLOCK_SIZE).step_by(4) {
            let lba = u32::deserialize(dvida_serialize::Endianness::Little, &buf[i as usize..])?.0;
            if lba == 0 {
                break;
            }

            // lba is the address of a double-indirect block
            self.free_double_indirect_block(lba as i64, cur_bitmap_lba, cur_buf)
                .await?;
        }

        // finally free the triple-indirect block itself
        self.free_block(block_lba, cur_bitmap_lba, cur_buf).await?;

        Ok(())
    }

    /// doesn't update the changes in the superblock to the filesystem
    pub async fn free_blocks(&mut self, inode: &mut InodePlus) -> Result<(), HalFsIOErr> {
        let mut cur_buf = Box::new([0u8; BLOCK_SIZE as usize]);
        let mut cur_bitmap_lba = 0;
        for i in 0..INODE_BLOCK_LIMIT as usize {
            if inode.inode.i_block[i] == 0 {
                return Ok(());
            }

            self.free_block(
                inode.inode.i_block[i] as i64,
                &mut cur_bitmap_lba,
                &mut cur_buf,
            )
            .await?;
        }

        if inode.inode.i_blocks > INODE_BLOCK_LIMIT {
            self.free_indirect_block(
                inode.inode.i_block[INODE_BLOCK_LIMIT as usize] as i64,
                &mut cur_bitmap_lba,
                &mut cur_buf,
            )
            .await?;
        }
        if inode.inode.i_blocks > INODE_IND_BLOCK_LIMIT {
            self.free_double_indirect_block(
                inode.inode.i_block[INODE_BLOCK_LIMIT as usize + 1] as i64,
                &mut cur_bitmap_lba,
                &mut cur_buf,
            )
            .await?;
        }
        if inode.inode.i_blocks > INODE_DOUBLE_IND_BLOCK_LIMIT {
            self.free_triple_indirect_block(
                inode.inode.i_block[INODE_BLOCK_LIMIT as usize + 2] as i64,
                &mut cur_bitmap_lba,
                &mut cur_buf,
            )
            .await?;
        }

        self.super_block.s_free_blocks_count -= inode.inode.i_blocks;

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

        self.write_inode(inode).await?;

        let inode_bitmap_lba = self
            .get_group(inode.group_number as i64)
            .get_inode_bitmap_lba();

        let mut buf = Box::new([0u8; BLOCK_SIZE as usize]);
        self.read_sectors(buf.clone(), inode_bitmap_lba).await?;

        buf[inode.relative_idx as usize / 8] =
            buf[inode.relative_idx as usize / 8] & !(1 << (inode.relative_idx % 8));

        self.write_sectors(buf.clone(), inode_bitmap_lba).await?;

        self.super_block.s_free_inodes_count += 1;

        buf.fill(0);

        self.super_block
            .serialize(dvida_serialize::Endianness::Little, &mut buf[0..])?;

        self.write_sectors(buf, RESERVED_BOOT_RECORD_OFFSET).await?;

        Ok(())
    }
}
