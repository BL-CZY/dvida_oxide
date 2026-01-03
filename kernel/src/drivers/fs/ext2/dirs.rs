use alloc::{boxed::Box, string::ToString};
use dvida_serialize::{DvDeserialize, DvSerialize};
use terminal::log;

use crate::{
    drivers::fs::ext2::{
        BLOCK_SIZE, DirEntry, DirEntryPartial, Inode, InodePlus,
        read::Progress,
        structs::{BlockIterElement, Ext2Fs},
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
        let mut buf: Box<[u8]> = self.get_buffer();
        let time = crate::time::formats::rtc_to_posix(
            &crate::time::Rtc::new()
                .read_datetime()
                .expect("Failed to get time"),
        );

        let mut blocks_iterator =
            self.create_block_iterator(&inode.inode, inode.group_number as i64);

        let mut entry = DirEntry::new(child_inode_idx, name.to_string());

        loop {
            let BlockIterElement {
                buf: buffer,
                is_terminated,
                block_idx,
            } = blocks_iterator.next(buf).await?;

            buf = buffer;
            if is_terminated {
                break;
            }

            let mut progr = 0;

            while progr < buf.len() {
                let entry_partial: &mut DirEntryPartial =
                    bytemuck::from_bytes_mut(&mut buf[progr..progr + size_of::<DirEntryPartial>()]);

                if entry_partial.rec_len + progr as u16 > self.super_block.block_size() as u16 {
                    return Err(HalFsIOErr::Corrupted);
                }

                // if it can fit, shrink this entry
                if entry_partial.rec_len - entry_partial.min_reclen() >= entry.record_length() {
                    log!(
                        "add_dir_entry: found entry that is long enough: {:?} for: {:?} with record length of: {:?}",
                        entry_partial,
                        entry,
                        entry.record_length()
                    );

                    entry.rec_len = entry_partial.rec_len as u16 - entry_partial.min_reclen();
                    entry_partial.rec_len = entry_partial.min_reclen();

                    let new_reclen = entry_partial.rec_len as usize;

                    entry.serialize(
                        dvida_serialize::Endianness::Little,
                        &mut buf[progr + new_reclen..],
                    )?;
                    self.io_handler.write_block(buf.clone(), block_idx).await?;

                    inode.inode.i_mtime = time;

                    return Ok(());
                }

                progr += entry_partial.rec_len as usize;
            }
        }

        // if we are here we need to allocate a new block
        self.expand_inode(&mut inode.inode, inode.group_number as i64, 1)
            .await?;

        entry.rec_len = self.super_block.block_size() as u16;

        let lba = self
            .get_block_lba(
                &inode.inode,
                inode.inode.i_size / self.super_block.block_size(),
            )
            .await?;

        buf = self.read_sectors(buf, lba as i64).await?;
        buf.fill(0);

        entry.serialize(dvida_serialize::Endianness::Little, &mut buf)?;

        inode.inode.i_size += self.super_block.block_size();
        self.write_sectors(buf, lba as i64).await?;

        inode.inode.i_mtime = time;

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

        if !self.is_dir_empty(&dir_inode).await? {
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
        let mut buf: Box<[u8]> = Box::new([0u8; BLOCK_SIZE as usize]);

        buf = self.read_sectors(buf, lba as i64).await?;

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
