use alloc::boxed::Box;
use dvida_serialize::{DvDeserialize, DvSerialize};

use crate::{
    drivers::fs::ext2::{INODE_SIZE, Inode, structs::Ext2Fs},
    hal::{fs::HalFsIOErr, storage::SECTOR_SIZE},
};

#[derive(Debug, Clone, Default)]
pub struct InodePlus {
    pub inode: Inode,
    pub absolute_idx: u32,
    pub group_number: u32,
    pub relative_idx: u32,
}

impl Ext2Fs {
    pub fn global_idx_to_inode_plus(&self, inode: Inode, idx: u32) -> InodePlus {
        InodePlus {
            inode,
            relative_idx: idx % self.super_block.s_inodes_per_group,
            group_number: idx / self.super_block.s_inodes_per_group,
            absolute_idx: idx,
        }
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
            absolute_idx: group_number * self.super_block.s_inodes_per_group + idx,
        }
    }

    pub async fn get_nth_inode(&self, idx: u32) -> Result<InodePlus, HalFsIOErr> {
        let group_number = idx / self.super_block.s_inodes_per_group;
        let offset = idx % self.super_block.s_inodes_per_group;

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
            absolute_idx: self.super_block.s_inodes_per_group * group_number + idx,
        })
    }

    pub async fn write_inode(&mut self, inode: &InodePlus) -> Result<(), HalFsIOErr> {
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

        Ok(())
    }
}
