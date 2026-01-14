use alloc::boxed::Box;
use dvida_serialize::DvDeserialize;
use terminal::log;

use crate::{
    drivers::fs::ext2::{
        DirEntry, DirEntryPartial, Inode, InodePlus,
        structs::{BlockIterElement, Ext2Fs},
    },
    hal::{
        fs::{HalFsIOErr, HalInode, OpenFlags, OpenFlagsValue},
        path::Path,
        storage::SECTOR_SIZE,
    },
};

pub const SUPERBLOCK_SIZE: i64 = 2;
pub const LBA_ADDR_LEN: usize = 4;

impl Ext2Fs {
    /// returns (Some(lba) if found, is_terminated)
    async fn find_entry_by_name_in_block(
        &mut self,
        name: &str,
        mut buf: Box<[u8]>,
        lba: i64,
        delete: bool,
        find_is_empty: bool,
        remaining_size: &mut u32,
    ) -> Result<(Option<i64>, bool, Box<[u8]>), HalFsIOErr> {
        let mut progr = 0;
        let mut this_entry_idx = 0;
        let mut last_entry_idx = 0;

        while progr < self.super_block.block_size() {
            let (entry, bytes_read) =
                DirEntry::deserialize(dvida_serialize::Endianness::Little, &buf[progr as usize..])?;

            log!("Read entry {:?} of size {}", entry, bytes_read);

            progr += bytes_read as u32;
            *remaining_size -= bytes_read as u32;
            let mut is_terminated = false;

            if *remaining_size <= 0 {
                is_terminated = true;
            }

            // skip the special entries "." and ".." when searching
            if entry.name.as_str() == "." || entry.name.as_str() == ".." {
                if is_terminated {
                    return Ok((None, true, buf));
                }
                last_entry_idx = this_entry_idx;
                this_entry_idx += bytes_read;
                continue;
            }

            // we don't check padding entries here
            if entry.inode != 0 && name == entry.name.as_str() && !find_is_empty {
                if delete {
                    // if this is the first entry
                    if this_entry_idx == last_entry_idx {
                        // set it to a padding entry
                        let raw_entry: &mut DirEntryPartial =
                            bytemuck::from_bytes_mut(&mut buf[0..size_of::<DirEntryPartial>()]);

                        raw_entry.inode = 0;
                        raw_entry.rec_len = entry.record_length();
                        raw_entry.name_len = 0;

                        self.write_sectors(buf.clone(), lba).await?;
                    } else {
                        let this_raw_entry: DirEntryPartial = *bytemuck::from_bytes(
                            &buf[this_entry_idx..this_entry_idx + size_of::<DirEntryPartial>()],
                        );

                        let last_raw_entry: &mut DirEntryPartial = bytemuck::from_bytes_mut(
                            &mut buf[last_entry_idx..last_entry_idx + size_of::<DirEntryPartial>()],
                        );

                        last_raw_entry.rec_len += this_raw_entry.rec_len;

                        self.write_sectors(buf.clone(), lba).await?;
                    }
                }
                log!("Found entry: {:?}", entry.inode);

                return Ok((Some(entry.inode as i64), is_terminated, buf));
            }

            if is_terminated {
                return Ok((None, false, buf));
            }

            last_entry_idx = this_entry_idx;
            this_entry_idx += bytes_read;
        }

        Ok((None, false, buf))
    }

    pub async fn find_entry_by_name(
        &mut self,
        name: &str,
        inode: &InodePlus,
    ) -> Result<Option<i64>, HalFsIOErr> {
        self.do_find_entry_by_name(name, inode, false, false).await
    }

    pub async fn find_entry_by_name_and_delete(
        &mut self,
        name: &str,
        inode: &InodePlus,
    ) -> Result<Option<i64>, HalFsIOErr> {
        self.do_find_entry_by_name(name, inode, true, false).await
    }

    // TODO: refactor this
    pub async fn is_dir_empty(&mut self, inode: &InodePlus) -> Result<bool, HalFsIOErr> {
        Ok(self
            .do_find_entry_by_name("", inode, false, false)
            .await?
            .is_none())
    }

    /// returns the index of the inode if the find_is_empty flag is not up
    /// otherwise, returns Some(1) if the directory is not empty
    pub async fn do_find_entry_by_name(
        &mut self,
        name: &str,
        victim_inode: &InodePlus,
        delete: bool,
        find_is_empty: bool,
    ) -> Result<Option<i64>, HalFsIOErr> {
        let inode = &victim_inode.inode;

        if !inode.is_directory() {
            return Err(HalFsIOErr::NotADirectory);
        }

        let mut remaining = inode.i_size;

        let mut buf: Box<[u8]> = self.get_buffer();

        let mut blocks_iterator =
            self.create_block_iterator(inode, victim_inode.group_number.into());

        loop {
            let BlockIterElement {
                buf: buffer,
                is_terminated,
                block_idx,
            } = blocks_iterator.next(buf).await?;
            if is_terminated {
                break;
            }

            buf = buffer;

            match self
                .find_entry_by_name_in_block(
                    name,
                    buf,
                    self.block_idx_to_lba(block_idx),
                    delete,
                    find_is_empty,
                    &mut remaining,
                )
                .await?
            {
                (res, true, _) => {
                    if find_is_empty {
                        return Ok(None);
                    } else {
                        return Ok(res);
                    }
                }
                (Some(res), false, _) => return Ok(Some(res)),
                (_, _, b) => buf = b,
            }
        }

        Ok(None)
    }

    /// takes in a path
    /// returns a tuple (the inode to the directory, Option<the inode to the file>)
    /// If the file doesn't exist the Option will be None
    pub async fn walk_path(
        &mut self,
        path: &Path,
    ) -> Result<(InodePlus, Option<InodePlus>), HalFsIOErr> {
        let group = self.get_group(0).await?;
        let inode_table_loc = group.get_inode_table_lba();

        let mut buf: Box<[u8]> = Box::new([0u8; SECTOR_SIZE]);
        buf = self.read_sectors(buf, inode_table_loc).await?;
        let temp = self.super_block.s_inode_size;
        log!("inode size: {:?}", temp);

        let mut inode = self.get_nth_inode(2).await?;

        log!("Root directory Inode: {:?}", inode);

        let mut directory_inode_idx = ROOT_DIRECTORY_INODE_IDX as u32;

        let mut file_inode: Option<InodePlus> = None;

        let mut it = path.normalize().components().into_iter().peekable();
        while let Some(component) = it.next() {
            log!("current component: {}", component);
            match self.find_entry_by_name(&component, &inode).await {
                Ok(Some(res)) => {
                    if it.peek().is_none() {
                        file_inode = Some(self.get_nth_inode(res as u32).await?);
                        break;
                    }

                    inode = self.get_nth_inode(res as u32).await?;

                    directory_inode_idx = res as u32;
                }
                Ok(None) => {
                    if it.peek().is_none() {
                        file_inode = None;
                    } else {
                        return Err(HalFsIOErr::NoSuchFileOrDirectory);
                    }
                }
                Err(e) => return Err(e),
            }
        }

        Ok((self.get_nth_inode(directory_inode_idx).await?, file_inode))
    }

    /// This function assumes that everything is initialized like the init function
    pub async fn open_file(
        &mut self,
        path: Path,
        flags: OpenFlags,
    ) -> Result<HalInode, HalFsIOErr> {
        let (mut directory_inode, file_inode) = self.walk_path(&path).await?;
        // remember whether the file existed before we attempt creation
        let existed = file_inode.is_some();

        let mut file_inode = if let Some(i) = file_inode {
            Some(i)
        } else {
            if flags.flags & OpenFlagsValue::CreateIfNotExist as i32 != 0 {
                let created = self
                    .create_file(
                        &mut directory_inode,
                        &path.file_name().ok_or(HalFsIOErr::BadPath)?,
                        flags.perms.ok_or(HalFsIOErr::NoPermsProvided)?,
                    )
                    .await?;

                Some(created)
            } else {
                return Err(HalFsIOErr::NoSuchFileOrDirectory);
            }
        };

        // If O_CREAT | O_EXCL and the file already existed, fail with FileExists
        if existed && (flags.flags & OpenFlagsValue::ErrorIfCreateFileExists as i32 != 0) {
            return Err(HalFsIOErr::FileExists);
        }

        Ok(HalInode::Ext2(file_inode.take().unwrap()))
    }
}

pub const ROOT_DIRECTORY_INODE_IDX: usize = 2;
