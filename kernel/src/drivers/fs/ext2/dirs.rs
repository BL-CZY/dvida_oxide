use alloc::{boxed::Box, string::ToString};
use dvida_serialize::{DvDeserialize, DvSerialize};

use crate::{
    drivers::fs::ext2::{
        BLOCK_SIZE, DirEntry, DirEntryPartial, Inode, InodePlus, read::Progress, structs::Ext2Fs,
    },
    hal::{
        fs::{DirEnt64, HalFsIOErr},
        path::Path,
    },
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

        // Read existing block if present. If LBA==0 treat as zero-filled (sparse)
        if lba != 0 {
            self.read_sectors(buf.clone(), lba).await?;
        } else {
            buf.fill(0);
        }

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

            // align up to next block boundary correctly
            dir.i_size = (dir.i_size + BLOCK_SIZE - 1) & !(BLOCK_SIZE - 1);

            // allocate one more block (pass bytes to expand)
            self.expand_inode(dir, inode.group_number as i64, BLOCK_SIZE as usize)
                .await?;

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

    pub async fn mkdir(&mut self, path: Path, perms: i32) -> Result<InodePlus, HalFsIOErr> {
        let (mut dir_inode, file_inode) = self.walk_path(&path).await?;

        if file_inode.is_some() {
            return Err(HalFsIOErr::FileExists);
        }

        Ok(self
            .create_inode(
                &mut dir_inode,
                &path.file_name().ok_or(HalFsIOErr::BadPath)?,
                true,
                perms,
            )
            .await?)
    }

    pub async fn rmdir(&mut self, path: Path) -> Result<(), HalFsIOErr> {
        let (mut dir_inode, file_inode) = self.walk_path(&path).await?;

        let Some(file_inode) = file_inode else {
            return Err(HalFsIOErr::NoSuchFileOrDirectory);
        };

        if !file_inode.inode.is_directory() {
            return Err(HalFsIOErr::NotADirectory);
        }

        if !self.is_dir_empty(&dir_inode.inode).await? {
            return Err(HalFsIOErr::DirectoryNotEmpty);
        }

        self.free_inode(&mut dir_inode).await?;

        Ok(())
    }

    // return (if it reaches the end of the entires or not, if it reaches the end of the buffer or not)
    async fn read_entries_till_next_block(
        &self,
        inode: &Inode,
        target: &mut [u8],
        offset: &mut i64,
        progress: &mut Progress,
    ) -> Result<(bool, bool), HalFsIOErr> {
        let lba = self.get_block_lba(inode, progress.block_idx as u32).await?;
        let buf = Box::new([0u8; BLOCK_SIZE as usize]);

        self.read_sectors(buf.clone(), lba as i64).await?;

        let mut progress_bytes = progress.offset as usize;
        while let Ok((entry, bytes_read)) =
            DirEntry::deserialize(dvida_serialize::Endianness::Little, &buf[progress_bytes..])
        {
            if entry.inode != 0 {
                let result_entry = DirEnt64 {
                    inode_idx: entry.inode as u64,
                    offset: *offset + bytes_read as i64,
                    file_type: entry.file_type as u8,
                    name: entry.name,
                };

                if result_entry.rec_len() + progress.bytes_written >= target.len() {
                    progress.block_idx += 1;
                    progress.offset = 0;

                    return Ok((false, true));
                }

                progress.bytes_written += result_entry.serialize(
                    dvida_serialize::Endianness::Little,
                    &mut target[progress.bytes_written as usize..],
                )?;
            }

            *offset += bytes_read as i64;
            progress_bytes += bytes_read;
        }

        if *offset >= inode.i_size.into() {
            return Ok((true, true));
        }

        progress.block_idx += 1;
        progress.offset = 0;

        Ok((false, false))
    }

    pub async fn iter_dir(
        &mut self,
        offset: &mut i64,
        mut buf: Box<[u8]>,
        inode: &mut InodePlus,
    ) -> Result<bool, HalFsIOErr> {
        if !inode.inode.is_directory() {
            return Err(HalFsIOErr::NotADirectory);
        }

        let mut progress = Progress {
            block_idx: *offset as u32 / self.super_block.block_size(),
            offset: *offset as u32 % self.super_block.block_size(),
            bytes_written: 0,
        };

        loop {
            let (reached_end, finished) = self
                .read_entries_till_next_block(&mut inode.inode, &mut buf, offset, &mut progress)
                .await?;

            if reached_end {
                return Ok(true);
            }

            if finished {
                break;
            }
        }

        Ok(false)
    }
}
