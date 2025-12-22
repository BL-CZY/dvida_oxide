use alloc::{boxed::Box, vec::Vec};
use dvida_serialize::{DvDeserialize, DvSerialize};

use crate::{
    crypto::iterators::{Bit, BitIterator},
    drivers::fs::ext2::{
        BLOCK_SIZE, INODE_SIZE, Inode,
        structs::{Ext2Fs, block_group_size},
    },
    hal::{fs::HalFsOpenErr, storage::SECTOR_SIZE},
    time::{Rtc, formats::rtc_to_posix},
};

pub const RESERVED_BOOT_RECORD_OFFSET: i64 = 2;
pub const BLOCK_SECTOR_SIZE: i64 = (BLOCK_SIZE as i64 / SECTOR_SIZE as i64) as i64;

pub struct AllocatedBlock {
    pub addr: i64,
    // relative to the block group
    pub block_idx: i64,
    pub gr_number: i64,
}

struct AllocatedInode {
    addr: i64,
    inode: Inode,
    // relative to the block group
    inode_idx: i64,
    gr_number: i64,
}

impl Ext2Fs {
    pub fn block_group_size(&self) -> i64 {
        block_group_size(
            self.super_block.s_blocks_per_group.into(),
            self.super_block.block_size().into(),
        )
    }

    pub async fn allocated_blocks_for_inode(
        &self,
        inode: &mut Inode,
        group_number: i64,
    ) -> Result<Vec<AllocatedBlock>, HalFsOpenErr> {
        if self.super_block.s_prealloc_blocks > 12 {
            return Err(HalFsOpenErr::Unsupported);
        }

        let blocks_allocated = self
            .allocate_n_blocks_in_group(group_number, self.super_block.s_prealloc_blocks as usize)
            .await?;

        for block in blocks_allocated.iter() {
            inode.i_block[inode.i_blocks as usize] = block.addr as u32;
            inode.i_blocks += 1;
        }

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

                return Ok(AllocatedInode {
                    addr: inode_count as i64 * INODE_SIZE as i64 + addr + 4 * BLOCK_SECTOR_SIZE,
                    inode_idx: idx as i64,
                    inode: ino,
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
        let mut buf = Box::new([0; BLOCK_SIZE as usize]);
        let mut cur_bitmap_lba = -1;

        for AllocatedBlock {
            block_idx,
            gr_number,
            ..
        } in blocks.iter()
        {
            let group = self.get_group(*gr_number);
            let block_bitmap_lba = group.get_block_bitmap_lba();

            if cur_bitmap_lba != block_bitmap_lba {
                if cur_bitmap_lba != -1 {
                    self.write_sectors(buf.clone(), cur_bitmap_lba).await?;
                }

                self.read_sectors(buf.clone(), block_bitmap_lba).await?;
                cur_bitmap_lba = block_bitmap_lba
            }

            let mut target = buf[*block_idx as usize / 8];
            target = target | 0x1 << *block_idx as usize % 8;
            buf[*block_idx as usize / 8] = target;
        }

        self.write_sectors(buf.clone(), cur_bitmap_lba).await?;

        let this_inode_lba_offset = (inode.inode_idx * INODE_SIZE) / BLOCK_SIZE as i64;
        let group = self.get_group(inode.gr_number);
        let lba = group.get_inode_table_lba() + this_inode_lba_offset;

        buf.fill(0);
        self.read_sectors(buf.clone(), lba).await?;

        let offset = (inode.inode_idx * INODE_SIZE) % BLOCK_SIZE as i64;
        inode.inode.serialize(
            dvida_serialize::Endianness::Little,
            &mut buf[offset as usize..],
        )?;

        self.write_sectors(buf, lba).await?;

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
