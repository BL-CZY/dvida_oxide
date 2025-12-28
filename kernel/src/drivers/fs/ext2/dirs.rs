use alloc::{boxed::Box, string::ToString};
use dvida_serialize::{DvDeserialize, DvSerialize};

use crate::{
    drivers::fs::ext2::{
        BLOCK_SIZE, DirEntry, Inode, InodePlus, read::INODE_TRIPLE_IND_BLOCK_LIMIT, structs::Ext2Fs,
    },
    hal::fs::HalFsIOErr,
};

impl Ext2Fs {
    pub async fn add_dir_entry(
        &mut self,
        dir: &mut Inode,
        group_number: u32,
        child_inode_addr: u32,
        name: &str,
    ) -> Result<(), HalFsIOErr> {
        let mut buf = Box::new([0u8; BLOCK_SIZE as usize]);

        'block_loop: for i in 0..INODE_TRIPLE_IND_BLOCK_LIMIT {
            let lba = self.get_block_lba(&dir, i).await? as i64;
            if lba == 0 {
                break;
            }

            self.read_sectors(buf.clone(), lba).await?;

            let mut progr = 0;
            'entry_loop: while let Ok((mut entry, bytes_read)) =
                DirEntry::deserialize(dvida_serialize::Endianness::Little, &buf[progr..])
            {
                if entry.inode == 0 {
                    entry.inode = child_inode_addr;
                    entry.name = name.to_string();

                    if entry.record_length() + progr as u16 >= BLOCK_SIZE as u16 {
                        entry.inode = 0;
                        entry.name = "".into();
                        entry.serialize_till_end(
                            dvida_serialize::Endianness::Little,
                            &mut buf[progr..],
                        )?;
                        self.write_sectors(buf.clone(), lba).await?;

                        self.expand_inode(dir, group_number as i64, 1).await?;

                        break 'entry_loop;
                    }

                    entry.serialize(dvida_serialize::Endianness::Little, &mut buf[progr..])?;
                    self.write_sectors(buf.clone(), lba).await?;
                    break 'block_loop;
                }
                progr += bytes_read;
            }
        }

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
