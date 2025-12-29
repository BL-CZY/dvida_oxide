use alloc::{boxed::Box, string::ToString};
use dvida_serialize::DvSerialize;

use crate::{
    drivers::fs::ext2::{BLOCK_SIZE, DirEntry, DirEntryPartial, Inode, InodePlus, structs::Ext2Fs},
    hal::fs::HalFsIOErr,
};

impl Ext2Fs {
    pub async fn add_dir_entry(
        &mut self,
        inode: &mut InodePlus,
        child_inode_idx: u32,
        name: &str,
    ) -> Result<(), HalFsIOErr> {
        let mut buf = Box::new([0u8; BLOCK_SIZE as usize]);
        let dir = &mut inode.inode;

        let block_idx = dir.i_size / BLOCK_SIZE;
        let offset = dir.i_size % BLOCK_SIZE;

        let lba = self.get_block_lba(dir, block_idx).await? as i64;

        let entry = DirEntry::new(child_inode_idx, name.to_string());

        if entry.record_length() + offset as u16 >= BLOCK_SIZE as u16 {
            let partial_entry = DirEntryPartial {
                inode: 0,
                rec_len: (BLOCK_SIZE - offset) as u16,
                name_len: 0,
            };

            partial_entry.serialize(
                dvida_serialize::Endianness::Little,
                &mut buf[offset as usize..],
            )?;
            self.write_sectors(buf.clone(), lba).await?;
            dir.i_size = (dir.i_size + BLOCK_SIZE) & !(BLOCK_SIZE - 1);

            self.expand_inode(dir, inode.group_number as i64, 1).await?;

            let lba = self.get_block_lba(dir, block_idx + 1).await? as i64;
            buf.fill(0);

            let bytes_written =
                entry.serialize(dvida_serialize::Endianness::Little, &mut buf[0..])?;
            self.write_sectors(buf.clone(), lba).await?;
            dir.i_size += bytes_written as u32;
        } else {
            let bytes_written = entry.serialize(
                dvida_serialize::Endianness::Little,
                &mut buf[offset as usize..],
            )?;
            self.write_sectors(buf.clone(), lba).await?;
            dir.i_size += bytes_written as u32;
        }

        self.write_inode(inode).await?;

        Ok(())
    }

    pub async fn mkdir(
        &mut self,
        inode: &mut InodePlus,
        name: &str,
        perms: i32,
    ) -> Result<InodePlus, HalFsIOErr> {
        Ok(self.create_inode(inode, name, false, perms).await?)
    }

    pub async fn rmdir() {}
    pub async fn iter_dir() {}
}
