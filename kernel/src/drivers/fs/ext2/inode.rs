use crate::log;
use alloc::boxed::Box;
use dvida_serialize::{DvDeserialize, DvSerialize};

use crate::{
    drivers::fs::ext2::{
        BLOCK_GROUP_DESCRIPTOR_SIZE, GroupDescriptor, INODE_SIZE, Inode, structs::Ext2Fs,
    },
    hal::{fs::HalFsIOErr, storage::SECTOR_SIZE},
};

#[derive(Debug, Clone, Default)]
pub struct InodePlus {
    pub inode: Inode,
    /// globally inode indicies start with 1
    pub absolute_idx: u32,
    pub group_number: u32,
    /// relatively this implementaiton will trait it to start with 0
    pub relative_idx: u32,
}

impl Ext2Fs {
    pub fn inode_block_count(&self, inode: &Inode) -> u32 {
        inode.i_blocks * (SECTOR_SIZE as u32) / self.super_block.block_size()
    }

    pub fn global_idx_to_inode_plus(&self, inode: Inode, idx: u32) -> InodePlus {
        let res = InodePlus {
            inode,
            relative_idx: (idx - 1) % self.super_block.s_inodes_per_group,
            group_number: (idx - 1) / self.super_block.s_inodes_per_group,
            absolute_idx: idx,
        };
        res
    }

    pub fn relative_idx_to_inode_plus(
        &self,
        inode: Inode,
        group_number: u32,
        idx: u32,
    ) -> InodePlus {
        InodePlus {
            inode,
            relative_idx: idx,
            group_number: group_number,
            absolute_idx: group_number * self.super_block.s_inodes_per_group + idx + 1,
        }
    }

    pub async fn get_nth_inode(&self, idx: u32) -> Result<InodePlus, HalFsIOErr> {
        let group_number = (idx - 1) / self.super_block.s_inodes_per_group;
        let offset = (idx - 1) % self.super_block.s_inodes_per_group;

        self.get_inode_in_group(group_number, offset).await
    }

    pub async fn get_inode_in_group(
        &self,
        group_number: u32,
        idx: u32,
    ) -> Result<InodePlus, HalFsIOErr> {
        let block_group = self.get_group(group_number as i64).await?;
        let lba = block_group.get_inode_table_lba();

        let sector_offset = (idx as i64 * INODE_SIZE as i64) / SECTOR_SIZE as i64;
        let byte_offset = (idx as i64 * INODE_SIZE as i64) % SECTOR_SIZE as i64;

        let mut buf: Box<[u8]> = Box::new([0u8; SECTOR_SIZE]);
        buf = self.read_sectors(buf, lba + sector_offset).await?;

        Ok(InodePlus {
            inode: Inode::deserialize(
                dvida_serialize::Endianness::Little,
                &buf[byte_offset as usize..],
            )?
            .0,
            group_number,
            relative_idx: idx,
            absolute_idx: self.super_block.s_inodes_per_group * group_number + idx + 1,
        })
    }

    pub async fn write_inode(&mut self, inode: &InodePlus) -> Result<(), HalFsIOErr> {
        self.do_write_inode(inode, false).await
    }

    pub async fn write_new_inode(&mut self, inode: &InodePlus) -> Result<(), HalFsIOErr> {
        self.do_write_inode(inode, true).await
    }

    pub async fn do_write_inode(
        &mut self,
        inode: &InodePlus,
        is_new: bool,
    ) -> Result<(), HalFsIOErr> {
        let block_group = self.get_group(inode.group_number as i64).await?;
        let lba = block_group.get_inode_table_lba();

        let sector_offset = (inode.relative_idx as i64 * INODE_SIZE as i64) / SECTOR_SIZE as i64;
        let byte_offset = (inode.relative_idx as i64 * INODE_SIZE as i64) % SECTOR_SIZE as i64;

        let mut buf: Box<[u8]> = Box::new([0u8; SECTOR_SIZE]);
        buf = self.read_sectors(buf, lba + sector_offset).await?;

        inode.inode.serialize(
            dvida_serialize::Endianness::Little,
            &mut buf[byte_offset as usize..],
        )?;

        self.write_sectors(buf.clone(), lba + sector_offset).await?;

        if is_new {
            let gr_number = inode.group_number as i64;
            let lba = self.get_block_group_table_lba();
            let lba_offset = (gr_number * BLOCK_GROUP_DESCRIPTOR_SIZE as i64) / SECTOR_SIZE as i64;
            let byte_offset = (gr_number * BLOCK_GROUP_DESCRIPTOR_SIZE as i64) % SECTOR_SIZE as i64;
            buf = self.read_sectors(buf, lba + lba_offset).await?;
            let descriptor: &mut GroupDescriptor = bytemuck::from_bytes_mut(
                &mut buf[byte_offset as usize..byte_offset as usize + size_of::<GroupDescriptor>()],
            );
            descriptor.bg_free_inodes_count -= 1;
            descriptor.bg_used_dirs_count += inode.inode.is_directory() as u16;
            self.write_sectors(buf.clone(), lba + lba_offset).await?;

            self.super_block.s_free_inodes_count -= 1;

            let super_block_bytes = bytemuck::bytes_of(&self.super_block);
            for i in 0..super_block_bytes.len() {
                buf[i] = super_block_bytes[i];
            }

            self.write_sectors(buf, 3).await?;
        }

        Ok(())
    }
}
