use alloc::{boxed::Box, vec, vec::Vec};
use dvida_serialize::DvDeserialize;

use crate::{
    crypto::iterators::{Bit, BitIterator},
    drivers::fs::ext2::{BLOCK_SIZE, INODE_SIZE, Inode, structs::Ext2Fs},
    hal::{fs::HalFsOpenErr, storage::SECTOR_SIZE},
    time::{Rtc, formats::rtc_to_posix},
};

pub const RESERVED_BOOT_RECORD_OFFSET: i64 = 2;
pub const BLOCK_SECTOR_SIZE: i64 = (BLOCK_SIZE as i64 / SECTOR_SIZE as i64) as i64;

struct AllocatedBlock {
    addr: i64,
    gr_number: i64,
}

struct AllocatedInode {
    addr: i64,
    gr_number: i64,
}

impl Ext2Fs {
    fn block_group_size(&self) -> i64 {
        self.super_block.s_blocks_per_group as i64 * (BLOCK_SIZE as i64 / SECTOR_SIZE as i64)
    }

    pub async fn traverse_find_blocks(
        &self,
        inode: &mut Inode,
        mut cur_empty_block_in_inode: usize,
        mut remaining_blocks: u8,
    ) -> Result<Vec<AllocatedBlock>, HalFsOpenErr> {
        let block_group_size = self.block_group_size();
        let mut blocks_allocated = vec![];

        for (group_number, addr) in (RESERVED_BOOT_RECORD_OFFSET..self.len() + 1)
            .step_by(block_group_size as usize)
            .enumerate()
        {
            if remaining_blocks <= 0 {
                break;
            }

            let mut buf = Box::new([0u8; BLOCK_SIZE as usize]);

            self.read_sectors(buf.clone(), addr + 2 * BLOCK_SECTOR_SIZE)
                .await?;

            let bit_iterator = BitIterator::new(buf.as_mut_slice());

            for (idx, bit) in bit_iterator.into_iter().enumerate() {
                if remaining_blocks <= 0 {
                    break;
                }

                if bit == Bit::One {
                    continue;
                }

                let block_addr = ((block_group_size * group_number as i64
                    + RESERVED_BOOT_RECORD_OFFSET
                    + 5
                    + BLOCK_SECTOR_SIZE)
                    + idx as i64) as u32;
                remaining_blocks -= 1;
                inode.i_block[cur_empty_block_in_inode] = block_addr;

                blocks_allocated.push(AllocatedBlock {
                    addr: block_addr as i64,
                    gr_number: group_number as i64,
                });

                cur_empty_block_in_inode += 1;
            }
        }

        Ok(blocks_allocated)
    }

    pub async fn allocated_blocks_for_inode(
        &self,
        inode: &mut Inode,
        group_number: i64,
    ) -> Result<Vec<AllocatedBlock>, HalFsOpenErr> {
        if self.super_block.s_prealloc_blocks > 12 {
            todo!("implement prealloc block more than 12");
        }

        let mut blocks_allocated = vec![];

        let mut remaining_blocks = self.super_block.s_prealloc_blocks;

        let block_group_size =
            self.super_block.s_blocks_per_group as i64 * (BLOCK_SIZE as i64 / SECTOR_SIZE as i64);

        let addr =
            block_group_size * group_number + RESERVED_BOOT_RECORD_OFFSET + 2 * BLOCK_SECTOR_SIZE;

        let mut buf = Box::new([0u8; BLOCK_SIZE as usize]);

        self.read_sectors(buf.clone(), addr).await?;

        let bit_iterator: BitIterator<u8> = BitIterator::new(buf.as_mut_slice());

        let mut cur_empty_block_in_inode = 0;
        for (idx, bit) in bit_iterator.into_iter().enumerate() {
            if remaining_blocks <= 0 {
                break;
            }

            if bit == Bit::One {
                continue;
            }

            let block_addr = ((block_group_size * group_number
                + RESERVED_BOOT_RECORD_OFFSET
                + 5
                + BLOCK_SECTOR_SIZE)
                + idx as i64) as u32;

            remaining_blocks -= 1;
            inode.i_block[cur_empty_block_in_inode] = block_addr;
            cur_empty_block_in_inode += 1;
            blocks_allocated.push(AllocatedBlock {
                addr: block_addr as i64,
                gr_number: group_number,
            });
        }

        if remaining_blocks <= 0 {
            return Ok(blocks_allocated);
        }

        // iterate over the entire filesystem
        blocks_allocated.extend(
            self.traverse_find_blocks(inode, cur_empty_block_in_inode, remaining_blocks)
                .await?
                .into_iter(),
        );

        Ok(blocks_allocated)
    }

    pub async fn find_available_inode(&self) -> Result<AllocatedInode, HalFsOpenErr> {
        let block_group_size = self.block_group_size();

        let mut inode_count = 0;
        let mut buf = Box::new([0u8; BLOCK_SIZE as usize]);

        for (group_number, addr) in (RESERVED_BOOT_RECORD_OFFSET..self.len() + 1)
            .step_by(block_group_size as usize)
            .enumerate()
        {
            let inode_bitmap_loc = addr + 3 * BLOCK_SECTOR_SIZE;
            self.read_sectors(buf.clone(), inode_bitmap_loc).await?;

            let bit_iterator: BitIterator<u8> = BitIterator::<u8> {
                num: buf.as_mut_slice(),
                idx: 0,
                bit_idx: 0,
            };

            for (idx, bit) in bit_iterator.into_iter().enumerate() {
                if inode_count as i64 * INODE_SIZE as i64 + addr + 4 * BLOCK_SECTOR_SIZE
                    < self.super_block.s_first_ino as i64
                {
                    continue;
                }

                inode_count += 1;

                if bit != Bit::Zero {
                    continue;
                }

                // TODO: set it to 1
                let inode_table_buf = Box::new([0u8; BLOCK_SIZE as usize]);
                let inode_table_loc = addr + 4;
                self.read_sectors(inode_table_buf.clone(), inode_table_loc)
                    .await?;

                let mut ino = Inode::deserialize(
                    dvida_serialize::Endianness::Little,
                    &inode_table_buf[idx * INODE_SIZE as usize..],
                )?
                .0;

                self.allocated_blocks_for_inode(&mut ino, group_number as i64)
                    .await?;

                //TODO: write everything
                return Ok(AllocatedInode {
                    addr: inode_count as i64 * INODE_SIZE as i64 + addr + 4 * BLOCK_SECTOR_SIZE,
                    gr_number: group_number as i64,
                });
            }
        }

        Err(HalFsOpenErr::NoAvailableInode)
    }

    async fn write_changes(
        &mut self,
        inode: &AllocatedInode,
        blocks: &[AllocatedBlock],
    ) -> Result<(), HalFsOpenErr> {
        Ok(())
    }

    pub async fn create_file(&mut self, dir: &mut Inode) -> Result<(), HalFsOpenErr> {
        if !dir.is_directory() {
            return Err(HalFsOpenErr::NotADirectory);
        }

        let time = Rtc::new()
            .read_datetime()
            .map_or_else(|| 0, |dt| rtc_to_posix(&dt));

        let allocated_inode = self.find_available_inode().await?;
        let mut inode = Inode {
            i_mode: 0b111111111, // TODO: permission
            i_uid: 0,            //TODO; uid
            i_size: self.super_block.s_prealloc_blocks as u32 * BLOCK_SIZE as u32, // TODO: directory
            i_atime: time,
            i_ctime: time,
            i_mtime: time,
            i_dtime: time, // TODO: deletion
            i_gid: 0,      // TODO: gid
            i_links_count: 1,
            i_blocks: self.super_block.s_prealloc_blocks as u32, // TODO: directory
            i_flags: 0,                                          // TODO: flags
            i_osd1: 0,
            i_osd2: [0; 12],
            i_block: [0; 15],
            i_file_acl: 0,
            i_dir_acl: 0,
            i_faddr: 0,
            i_generation: 0,
        };
        let blocks = self
            .allocated_blocks_for_inode(&mut inode, allocated_inode.gr_number)
            .await?;

        self.write_changes(&allocated_inode, &blocks).await?;

        Ok(())
    }
}
