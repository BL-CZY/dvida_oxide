use core::cmp::min;

use alloc::boxed::Box;
use dvida_serialize::{DvDeserialize, DvSerialize};
use terminal::log;

use crate::{
    drivers::fs::ext2::{
        BLOCK_SIZE, DirEntry, DirEntryPartial, INODE_SIZE, Inode, InodePlus,
        read::INODE_BLOCK_LIMIT, structs::Ext2Fs,
    },
    hal::{
        fs::{HalFsIOErr, HalInode, OpenFlags, OpenFlagsValue},
        path::Path,
        storage::SECTOR_SIZE,
    },
};

pub const SUPERBLOCK_SIZE: i64 = 2;
pub const LBA_ADDR_LEN: usize = 4;

struct IndBlockIter {
    buf: Box<[u8]>,
    acc: usize,
}

impl Iterator for IndBlockIter {
    type Item = i64;

    /// it skips everything that is 0
    fn next(&mut self) -> Option<Self::Item> {
        let mut lba = 0;

        while lba == 0 {
            if self.acc >= BLOCK_SIZE as usize {
                return None;
            }

            lba =
                Self::Item::deserialize(dvida_serialize::Endianness::Little, &self.buf[self.acc..])
                    .ok()?
                    .0;

            self.acc += LBA_ADDR_LEN;
        }

        Some(lba as i64)
    }
}

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

        while let Ok((entry, bytes_read)) =
            DirEntry::deserialize(dvida_serialize::Endianness::Little, &buf[progr..])
        {
            log!("Read entry {:?} of size {}", entry, bytes_read);
            progr += bytes_read;
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
                        let raw_entry = DirEntryPartial {
                            inode: 0,
                            rec_len: entry.record_length(),
                            name_len: 0,
                        };

                        raw_entry.serialize(dvida_serialize::Endianness::Little, &mut buf[0..])?;
                        self.write_sectors(buf.clone(), lba).await?;
                    } else {
                        let this_raw_entry = DirEntryPartial::deserialize(
                            dvida_serialize::Endianness::Little,
                            &mut buf[this_entry_idx..],
                        )?
                        .0;

                        let mut last_raw_entry = DirEntryPartial::deserialize(
                            dvida_serialize::Endianness::Little,
                            &mut buf[last_entry_idx..],
                        )?
                        .0;

                        last_raw_entry.rec_len += this_raw_entry.rec_len;

                        last_raw_entry.serialize(
                            dvida_serialize::Endianness::Little,
                            &mut buf[last_entry_idx..],
                        )?;

                        this_raw_entry.serialize(
                            dvida_serialize::Endianness::Little,
                            &mut buf[this_entry_idx..],
                        )?;

                        self.write_sectors(buf.clone(), lba).await?;
                    }
                }

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
        inode: &Inode,
    ) -> Result<Option<i64>, HalFsIOErr> {
        self.do_find_entry_by_name(name, inode, false, false).await
    }

    pub async fn find_entry_by_name_and_delete(
        &mut self,
        name: &str,
        inode: &Inode,
    ) -> Result<Option<i64>, HalFsIOErr> {
        self.do_find_entry_by_name(name, inode, true, false).await
    }

    // TODO: refactor this
    pub async fn is_dir_empty(&mut self, inode: &Inode) -> Result<bool, HalFsIOErr> {
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
        inode: &Inode,
        delete: bool,
        find_is_empty: bool,
    ) -> Result<Option<i64>, HalFsIOErr> {
        if !inode.is_directory() {
            return Err(HalFsIOErr::NotADirectory);
        }

        let mut remaining = inode.i_size;

        let mut buf: Box<[u8]> = Box::new([0u8; BLOCK_SIZE as usize]);

        // compute number of data blocks represented by i_blocks (which counts 512-byte sectors)
        let sectors_per_block = (BLOCK_SIZE as usize) / (SECTOR_SIZE as usize);
        let block_count = (inode.i_blocks as usize) / sectors_per_block;

        // the direct blocks (0..INODE_BLOCK_LIMIT-1)
        for i in 0..min(block_count, INODE_BLOCK_LIMIT as usize) {
            log!("checking block {} of inode", i);
            let lba = self.block_idx_to_lba(inode.i_block[i]);
            if lba == 0 {
                continue;
            }
            buf = self.read_sectors(buf, lba).await?;

            match self
                .find_entry_by_name_in_block(name, buf, lba, delete, find_is_empty, &mut remaining)
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

        // single indirect (index 12)
        let lba = inode.i_block[12] as i64;

        if lba != 0 {
            buf = self.read_sectors(buf, lba).await?;

            let iterator = IndBlockIter {
                buf: buf.clone(),
                acc: 0,
            };

            let mut ind_buf: Box<[u8]> = Box::new([0u8; BLOCK_SIZE as usize]);
            for addr in iterator.into_iter() {
                if addr == 0 {
                    continue;
                }
                ind_buf = self.read_sectors(ind_buf, addr).await?;

                match self
                    .find_entry_by_name_in_block(
                        name,
                        ind_buf.clone(),
                        addr,
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
        }

        // double indirect (index 13)
        let lba = inode.i_block[13] as i64;

        if lba != 0 {
            buf = self.read_sectors(buf, lba).await?;

            let iterator = IndBlockIter {
                buf: buf.clone(),
                acc: 0,
            };

            let mut ind_buf: Box<[u8]> = Box::new([0u8; BLOCK_SIZE as usize]);
            let mut ind_ind_buf: Box<[u8]> = Box::new([0u8; BLOCK_SIZE as usize]);
            for addr in iterator.into_iter() {
                if addr == 0 {
                    continue;
                }
                ind_buf = self.read_sectors(ind_buf, addr).await?;

                let ind_iterator = IndBlockIter {
                    buf: ind_buf.clone(),
                    acc: 0,
                };

                for ind_addr in ind_iterator.into_iter() {
                    if ind_addr == 0 {
                        continue;
                    }
                    ind_ind_buf = self.read_sectors(ind_ind_buf, ind_addr).await?;
                    match self
                        .find_entry_by_name_in_block(
                            name,
                            ind_ind_buf.clone(),
                            ind_addr,
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
            }

            // triple indirect (index 14)
            let lba = inode.i_block[14] as i64;

            if lba != 0 {
                buf = self.read_sectors(buf, lba).await?;
                let iterator = IndBlockIter {
                    buf: buf.clone(),
                    acc: 0,
                };

                let mut ind_ind_ind_buf: Box<[u8]> = Box::new([0u8; BLOCK_SIZE as usize]);

                for addr in iterator.into_iter() {
                    if addr == 0 {
                        continue;
                    }
                    ind_buf = self.read_sectors(ind_buf, addr).await?;

                    let ind_iterator = IndBlockIter {
                        buf: ind_buf.clone(),
                        acc: 0,
                    };

                    for ind_addr in ind_iterator.into_iter() {
                        if ind_addr == 0 {
                            continue;
                        }
                        ind_ind_buf = self.read_sectors(ind_ind_buf, ind_addr).await?;

                        let ind_ind_iterator = IndBlockIter {
                            buf: ind_ind_buf.clone(),
                            acc: 0,
                        };

                        for ind_ind_addr in ind_ind_iterator.into_iter() {
                            if ind_ind_addr == 0 {
                                continue;
                            }
                            ind_ind_ind_buf =
                                self.read_sectors(ind_ind_ind_buf, ind_ind_addr).await?;
                            match self
                                .find_entry_by_name_in_block(
                                    name,
                                    ind_ind_ind_buf.clone(),
                                    ind_ind_addr,
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
                    }
                }
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
        log!("inode size: {:?}", self.super_block.s_inode_size);

        let mut inode = Inode::deserialize(
            dvida_serialize::Endianness::Little,
            &buf[self.super_block.s_inode_size as usize..],
        )?
        .0;

        log!("Root directory Inode: {:?}", inode);

        let mut directory_inode_idx = ROOT_DIRECTORY_INODE_IDX as u32;

        let mut file_inode: Option<InodePlus> = None;

        let mut it = path.normalize().components().into_iter().peekable();
        while let Some(component) = it.next() {
            log!("current component: {}", component);
            match self.find_entry_by_name(&component, &inode).await {
                Ok(Some(res)) => {
                    buf = self.read_sectors(buf, res).await?;

                    if it.peek().is_none() {
                        file_inode = Some(
                            self.global_idx_to_inode_plus(
                                Inode::deserialize(
                                    dvida_serialize::Endianness::Little,
                                    buf.as_ref(),
                                )?
                                .0,
                                res as u32,
                            ),
                        );
                        break;
                    }

                    inode =
                        Inode::deserialize(dvida_serialize::Endianness::Little, buf.as_ref())?.0;

                    directory_inode_idx = res as u32;
                }
                Ok(None) => return Err(HalFsIOErr::NoSuchFileOrDirectory),
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

pub const ROOT_DIRECTORY_INODE_IDX: usize = 1;
