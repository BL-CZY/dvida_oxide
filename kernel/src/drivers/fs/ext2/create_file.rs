use alloc::{boxed::Box, collections::btree_map::BTreeMap, vec::Vec};
use terminal::log;

use crate::{
    drivers::fs::ext2::{
        BLOCK_GROUP_DESCRIPTOR_SIZE, BLOCK_SIZE, GroupDescriptor, Inode, InodePlus,
        structs::{Ext2Fs, block_group_size},
    },
    hal::{fs::HalFsIOErr, storage::SECTOR_SIZE},
    time::{Rtc, formats::rtc_to_posix},
};

pub const RESERVED_BOOT_RECORD_OFFSET: i64 = 2;
pub const BLOCK_SECTOR_SIZE: i64 = (BLOCK_SIZE as i64 / SECTOR_SIZE as i64) as i64;

pub struct AllocatedBlock {
    pub addr: i64,
    // relative to the block group
    pub block_idx: i64,
    pub gr_number: i64,
}

impl Ext2Fs {
    pub fn block_group_size(&self) -> i64 {
        block_group_size(
            self.super_block.s_blocks_per_group.into(),
            self.super_block.block_size().into(),
        )
    }

    pub async fn allocated_blocks_for_new_inode(
        &self,
        inode: &mut Inode,
        group_number: i64,
        num: usize,
    ) -> Result<Vec<AllocatedBlock>, HalFsIOErr> {
        if num > 12 {
            return Err(HalFsIOErr::Unsupported);
        }

        let blocks_allocated = self
            .allocate_n_blocks_in_group(group_number, num as usize)
            .await?;

        for (idx, block) in blocks_allocated.iter().enumerate() {
            inode.i_block[idx] = block.addr as u32;
        }
        inode.i_blocks += blocks_allocated.len() as u32 * BLOCK_SIZE / SECTOR_SIZE as u32;

        Ok(blocks_allocated)
    }

    pub async fn find_available_inode(&self) -> Result<InodePlus, HalFsIOErr> {
        let group_count = self.super_block.block_groups_count();

        let cur_lba = 0;
        let mut buf: Box<[u8]> = self.get_buffer();
        let table_lba = self.get_block_group_table_lba();

        for group_idx in 0..group_count {
            let lba_offset =
                (group_idx as i64 * BLOCK_GROUP_DESCRIPTOR_SIZE as i64) / SECTOR_SIZE as i64;

            let lba = table_lba + lba_offset;
            if cur_lba != lba {
                buf = self.read_sectors(buf, lba).await?;
            }

            let block_group = self.get_group_from_buffer(group_idx as i64, &buf)?;

            if block_group.descriptor.bg_free_inodes_count == 0 {
                continue;
            }

            let mut bitmap_buf = self.get_buffer();
            bitmap_buf = self
                .read_sectors(bitmap_buf, block_group.get_inode_bitmap_lba())
                .await?;

            for idx in 0..self.super_block.s_inodes_per_group as usize {
                if bitmap_buf[idx / 8] & 0x1 << (idx % 8) == 0 {
                    let ino = Inode::default();

                    return Ok(self.relative_idx_to_inode_plus(ino, group_idx, idx as u32));
                }
            }
        }

        Err(HalFsIOErr::NoAvailableInode)
    }

    pub async fn write_newly_allocated_blocks(
        &mut self,
        mut buf: Box<[u8]>,
        blocks: &[AllocatedBlock],
    ) -> Result<(), HalFsIOErr> {
        let mut cur_bitmap_lba = -1;

        let mut allocated_blocks_map: BTreeMap<i64, i64> = BTreeMap::new();

        for AllocatedBlock {
            block_idx,
            gr_number,
            ..
        } in blocks.iter()
        {
            allocated_blocks_map
                .entry(*gr_number)
                .and_modify(|v| *v += 1)
                .or_insert(1);

            let group = self.get_group(*gr_number).await?;
            let block_bitmap_lba = group.get_block_bitmap_lba();

            if cur_bitmap_lba != block_bitmap_lba {
                if cur_bitmap_lba != -1 {
                    self.write_sectors(buf.clone(), cur_bitmap_lba).await?;
                }

                buf = self.read_sectors(buf, block_bitmap_lba).await?;
                cur_bitmap_lba = block_bitmap_lba
            }

            let mut target = buf[*block_idx as usize / 8];
            target = target | 0x1 << *block_idx as usize % 8;
            buf[*block_idx as usize / 8] = target;
        }

        self.write_sectors(buf.clone(), cur_bitmap_lba).await?;

        let mut cur_group_buffer_lba = -1;
        for (group_idx, num_allocated) in allocated_blocks_map {
            let bg_table_block_idx = self.super_block.s_first_data_block + 1;
            let lba = self.block_idx_to_lba(bg_table_block_idx);
            let lba_offset = (group_idx * BLOCK_GROUP_DESCRIPTOR_SIZE as i64) / SECTOR_SIZE as i64;
            let byte_offset = (group_idx * BLOCK_GROUP_DESCRIPTOR_SIZE as i64) % SECTOR_SIZE as i64;

            if lba + lba_offset != cur_group_buffer_lba {
                if cur_group_buffer_lba != -1 {
                    self.write_sectors(buf.clone(), cur_group_buffer_lba)
                        .await?;
                }

                cur_group_buffer_lba = lba + lba_offset;
                buf = self.read_sectors(buf, lba + lba_offset).await?;
            }

            let descriptor: &mut GroupDescriptor = bytemuck::from_bytes_mut(
                &mut buf[byte_offset as usize..byte_offset as usize + size_of::<GroupDescriptor>()],
            );

            descriptor.bg_free_blocks_count -= num_allocated as u16;
        }

        self.write_sectors(buf, cur_group_buffer_lba).await?;

        Ok(())
    }

    async fn write_changes(
        &mut self,
        inode: &InodePlus,
        blocks: &[AllocatedBlock],
    ) -> Result<(), HalFsIOErr> {
        let buf = self.get_buffer();
        self.write_newly_allocated_blocks(buf, blocks).await?;

        self.write_new_inode(inode).await?;

        Ok(())
    }

    pub async fn create_file(
        &mut self,
        inode: &mut InodePlus,
        name: &str,
        perms: i32,
    ) -> Result<InodePlus, HalFsIOErr> {
        Ok(self.create_inode(inode, name, false, perms).await?)
    }

    pub async fn create_inode(
        &mut self,
        dir_inode: &mut InodePlus,
        name: &str,
        is_dir: bool,
        perms: i32,
    ) -> Result<InodePlus, HalFsIOErr> {
        if name.len() > 255 {
            return Err(HalFsIOErr::NameTooLong);
        }

        log!("Creating inode under: {:?}", dir_inode);

        let dir = &dir_inode.inode;

        if !dir.is_directory() {
            return Err(HalFsIOErr::NotADirectory);
        }

        let time = Rtc::new()
            .read_datetime()
            .map_or_else(|| 0, |dt| rtc_to_posix(&dt));

        let mut allocated_inode = self.find_available_inode().await?;

        log!("Found availble inode: {:?}", allocated_inode);

        let inode = &mut allocated_inode.inode;

        // Set mode and file type bits (directory vs regular file)
        const S_IFDIR: u16 = 0x4000;
        const S_IFREG: u16 = 0x8000;
        inode.i_mode = (perms as u16) | if is_dir { S_IFDIR } else { S_IFREG };
        inode.i_uid = 0; // TODO: support UID
        inode.i_size = 0;
        inode.i_atime = time;
        inode.i_ctime = time;
        inode.i_mtime = time;
        inode.i_dtime = 0;
        inode.i_gid = 0; // TODO: support GID
        inode.i_links_count = if is_dir { 2 } else { 1 };
        inode.i_blocks = 0;
        inode.i_flags = 0;
        inode.i_osd1 = 0;
        inode.i_osd2 = [0; 12];
        inode.i_block = [0; 15];
        inode.i_file_acl = 0;
        inode.i_dir_acl = 0;
        inode.i_faddr = 0;
        inode.i_generation = 0;

        let blocks = self
            .allocated_blocks_for_new_inode(
                inode,
                allocated_inode.group_number.into(),
                if is_dir {
                    self.super_block.s_prealloc_dir_blocks as usize
                } else {
                    self.super_block.s_prealloc_blocks as usize
                },
            )
            .await?;

        self.add_dir_entry(dir_inode, allocated_inode.absolute_idx as u32, name)
            .await?;

        if is_dir {
            let temp = allocated_inode.absolute_idx as u32;
            self.add_dir_entry(&mut allocated_inode, temp, ".").await?;
            self.add_dir_entry(&mut allocated_inode, dir_inode.absolute_idx as u32, "..")
                .await?;

            dir_inode.inode.i_links_count = dir_inode.inode.i_links_count.saturating_add(1);
            self.write_inode(dir_inode).await?;
        }

        self.write_changes(&allocated_inode, &blocks).await?;

        Ok(allocated_inode)
    }
}
