use core::cmp::min;

use alloc::{boxed::Box, string::String};
use dvida_serialize::DvDeserialize;

use crate::{
    drivers::fs::ext2::{DirEntry, INODE_SIZE, Inode, structs::Ext2Fs},
    hal::{
        fs::{HalFsOpenErr, HalInode, OpenFlags},
        path::Path,
        storage::SECTOR_SIZE,
    },
};

const SUPERBLOCK_SIZE: i64 = 2;
const DIR_INDIRECT_BLOCK_START: usize = 13;

impl Ext2Fs {
    pub async fn find_entry_by_name(&self, name: &str, inode: Inode) -> Result<(), HalFsOpenErr> {
        if !inode.is_directory() {
            return Err(HalFsOpenErr::NoSuchFileOrDirectory);
        }

        let mut buf = Box::new([0u8; 512]);
        for i in 0..min(inode.i_blocks as usize, DIR_INDIRECT_BLOCK_START) {
            buf.fill(0);
            let lba = inode.i_block[i] as i64;
            self.read_sectors(buf.clone(), lba).await?;

            let entry: DirEntry =
                DirEntry::deserialize(dvida_serialize::Endianness::Little, buf.as_ref())?.0;

            let name = buf[]
        }

        Ok(())
    }

    /// This function assumes that everything is initialized like the init function
    pub async fn open_file(&self, path: Path, flags: OpenFlags) -> Result<HalInode, HalFsOpenErr> {
        let block_size = self.super_block.block_size();
        let block_per_group = self.super_block.s_blocks_per_group;
        let superblock_loc = self.super_block.s_first_data_block;
        let inode_table_loc = superblock_loc as i64 + (block_size as i64 / SECTOR_SIZE as i64) * 3;

        let buf = Box::new([0u8; 512]);
        let inode_table = self.read_sectors(buf.clone(), inode_table_loc).await?;

        let inode = Inode::deserialize(
            dvida_serialize::Endianness::Little,
            &buf[INODE_SIZE as usize..],
        )?
        .0;

        for component in path.components().into_iter() {}

        Ok(HalInode::Foo)
    }
}
