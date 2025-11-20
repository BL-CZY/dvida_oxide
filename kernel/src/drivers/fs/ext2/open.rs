use crate::{
    drivers::fs::ext2::structs::Ext2Fs,
    hal::{
        fs::{HalFsOpenErr, HalInode, OpenFlags},
        path::Path,
        storage::SECTOR_SIZE,
    },
};

const SUPERBLOCK_SIZE: i64 = 2;

impl Ext2Fs {
    /// This function assumes that everything is initialized like the init function
    pub async fn open_file(&self, path: Path, flags: OpenFlags) -> Result<HalInode, HalFsOpenErr> {
        let block_size = self.super_block.block_size();
        let block_per_group = self.super_block.s_blocks_per_group;
        let superblock_loc = self.super_block.s_first_data_block;
        let inode_table_loc = superblock_loc as i64 + (block_size as i64 / SECTOR_SIZE as i64) * 3;

        let buf = [0u8; 512];

        for component in path.components().into_iter() {}

        Ok(HalInode::Foo)
    }
}
