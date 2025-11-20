use crate::{
    drivers::fs::ext2::{Inode, structs::Ext2Fs},
    hal::{fs::OpenFlags, path::Path},
};

impl Ext2Fs {
    pub fn open_file(&self, path: Path, flags: OpenFlags) -> Result<Inode, Ext2Err> {
        for component in path.components().into_iter() {}

        Ok(Inode::default())
    }
}
