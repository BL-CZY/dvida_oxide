use core::cmp::min;

use alloc::boxed::Box;
use dvida_serialize::DvDeserialize;

use crate::{
    drivers::fs::ext2::{BLOCK_SIZE, DirEntry, INODE_SIZE, Inode, InodePlus, structs::Ext2Fs},
    hal::{
        fs::{HalFsIOErr, HalInode, OpenFlags, OpenFlagsValue},
        path::Path,
        storage::SECTOR_SIZE,
    },
};

pub const SUPERBLOCK_SIZE: i64 = 2;
pub const DIR_INDIRECT_BLOCK_START: usize = 13;
pub const LBA_ADDR_LEN: usize = 4;

struct DirEntryIter {
    buf: Box<[u8; BLOCK_SIZE as usize]>,
    acc: usize,
}

impl Iterator for DirEntryIter {
    type Item = DirEntry;

    fn next(&mut self) -> Option<Self::Item> {
        let (entry, read) =
            Self::Item::deserialize(dvida_serialize::Endianness::Little, &self.buf[self.acc..])
                .ok()?;

        self.acc += read;

        if entry.inode == 0 { None } else { Some(entry) }
    }
}

struct IndBlockIter {
    buf: Box<[u8; BLOCK_SIZE as usize]>,
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
    fn find_entry_by_name_in_block(
        &self,
        name: &str,
        buf: Box<[u8; BLOCK_SIZE as usize]>,
    ) -> Option<i64> {
        let entry_iter = DirEntryIter {
            buf: buf.clone(),
            acc: 0,
        };

        for entry in entry_iter.into_iter() {
            if name == entry.name.as_str() {
                return Some(entry.inode as i64);
            }
        }

        None
    }

    /// returns the LBA of the inode
    pub async fn find_entry_by_name(
        &self,
        name: &str,
        inode: &Inode,
    ) -> Result<Option<i64>, HalFsIOErr> {
        if !inode.is_directory() {
            return Err(HalFsIOErr::NotADirectory);
        }

        let buf = Box::new([0u8; BLOCK_SIZE as usize]);

        // the first 12
        for i in 0..min(inode.i_blocks as usize, DIR_INDIRECT_BLOCK_START) {
            let lba = inode.i_block[i] as i64;
            self.read_sectors(buf.clone(), lba).await?;

            match self.find_entry_by_name_in_block(name, buf.clone()) {
                Some(res) => return Ok(Some(res)),
                _ => {}
            }
        }

        // the 13th
        let lba = inode.i_block[13] as i64;

        if lba != 0 {
            self.read_sectors(buf.clone(), lba).await?;

            let iterator = IndBlockIter {
                buf: buf.clone(),
                acc: 0,
            };

            let ind_buf = Box::new([0u8; BLOCK_SIZE as usize]);
            for addr in iterator.into_iter() {
                self.read_sectors(ind_buf.clone(), addr).await?;

                match self.find_entry_by_name_in_block(name, ind_buf.clone()) {
                    Some(res) => return Ok(Some(res)),
                    _ => {}
                }
            }

            // the 14th
            let lba = inode.i_block[14] as i64;

            if lba != 0 {
                self.read_sectors(buf.clone(), lba).await?;

                let iterator = IndBlockIter {
                    buf: buf.clone(),
                    acc: 0,
                };

                let ind_ind_buf = Box::new([0u8; BLOCK_SIZE as usize]);
                for addr in iterator.into_iter() {
                    self.read_sectors(ind_buf.clone(), addr).await?;

                    let ind_iterator = IndBlockIter {
                        buf: ind_buf.clone(),
                        acc: 0,
                    };

                    for ind_addr in ind_iterator.into_iter() {
                        self.read_sectors(ind_ind_buf.clone(), ind_addr).await?;
                        match self.find_entry_by_name_in_block(name, ind_ind_buf.clone()) {
                            Some(res) => return Ok(Some(res)),
                            _ => {}
                        }
                    }
                }

                // the 15th
                let lba = inode.i_block[15] as i64;

                if lba != 0 {
                    self.read_sectors(buf.clone(), lba).await?;
                    let iterator = IndBlockIter {
                        buf: buf.clone(),
                        acc: 0,
                    };

                    let ind_ind_ind_buf = Box::new([0u8; BLOCK_SIZE as usize]);

                    for addr in iterator.into_iter() {
                        self.read_sectors(ind_buf.clone(), addr).await?;

                        let ind_iterator = IndBlockIter {
                            buf: ind_buf.clone(),
                            acc: 0,
                        };

                        for ind_addr in ind_iterator.into_iter() {
                            self.read_sectors(ind_ind_buf.clone(), ind_addr).await?;

                            let ind_ind_iterator = IndBlockIter {
                                buf: ind_ind_buf.clone(),
                                acc: 0,
                            };

                            for ind_ind_addr in ind_ind_iterator.into_iter() {
                                self.read_sectors(ind_ind_ind_buf.clone(), ind_ind_addr)
                                    .await?;
                                match self
                                    .find_entry_by_name_in_block(name, ind_ind_ind_buf.clone())
                                {
                                    Some(res) => return Ok(Some(res)),
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    pub fn get_inode_idx(&self, inode_lba: u32) -> u32 {
        let block_group = self.get_group_from_lba(inode_lba as i64);
        let inode_table_lba = block_group.get_inode_table_lba();
        let inode_idx = (inode_lba - inode_table_lba as u32) / INODE_SIZE as u32;
        inode_idx
    }

    /// takes in a path
    /// returns a tuple (the inode to the directory, Option<the inode to the file>)
    /// If the file doesn't exist the Option will be None
    pub async fn walk_path(
        &self,
        path: Path,
    ) -> Result<(InodePlus, Option<InodePlus>), HalFsIOErr> {
        let block_size = self.super_block.block_size();
        let superblock_loc = self.super_block.s_first_data_block;
        let inode_table_loc = superblock_loc as i64 + (block_size as i64 / SECTOR_SIZE as i64) * 3;

        let buf = Box::new([0u8; 512]);
        self.read_sectors(buf.clone(), inode_table_loc).await?;

        let mut inode = Inode::deserialize(
            dvida_serialize::Endianness::Little,
            &buf[INODE_SIZE as usize..],
        )?
        .0;
        let mut directory_inode_lba = inode_table_loc as u32 + INODE_SIZE as u32;
        let mut directory_block_group_idx = 0;

        let mut file_inode: Option<InodePlus> = None;

        let mut it = path.normalize().components().into_iter().peekable();
        while let Some(component) = it.next() {
            match self.find_entry_by_name(&component, &inode).await {
                Ok(Some(res)) => {
                    self.read_sectors(buf.clone(), res).await?;

                    if it.peek().is_none() {
                        file_inode = Some(InodePlus {
                            inode: Inode::deserialize(
                                dvida_serialize::Endianness::Little,
                                buf.as_ref(),
                            )?
                            .0,
                            idx: self.get_inode_idx(res as u32),
                            group_number: self.get_group_from_lba(res).group_number as u32,
                        });
                        continue;
                    }

                    inode =
                        Inode::deserialize(dvida_serialize::Endianness::Little, buf.as_ref())?.0;

                    directory_inode_lba = res as u32;
                    directory_block_group_idx = self.get_group_from_lba(res).group_number;
                }
                Ok(None) => return Err(HalFsIOErr::NoSuchFileOrDirectory),
                Err(e) => return Err(e),
            }
        }

        Ok((
            InodePlus {
                inode,
                group_number: directory_block_group_idx as u32,
                idx: self.get_inode_idx(directory_inode_lba) as u32,
            },
            file_inode,
        ))
    }

    /// This function assumes that everything is initialized like the init function
    pub async fn open_file(&self, path: Path, flags: OpenFlags) -> Result<HalInode, HalFsIOErr> {
        let (directory_inode, file_inode) = self.walk_path(path).await?;

        let Some(file_inode) = file_inode else {
            if flags.flags & OpenFlagsValue::CreateIfNotExist as i32 != 0 {
                self.create_file(&mut directory_inode).await?;
                todo!()
            } else {
                return Err(HalFsIOErr::NoSuchFileOrDirectory);
            }
        };

        if flags.flags & OpenFlagsValue::ErrorIfCreateFileExists as i32 != 0 {
            return Err(HalFsIOErr::FileExists);
        }

        Ok(HalInode::Ext2(file_inode))
    }
}
