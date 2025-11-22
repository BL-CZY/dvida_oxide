use alloc::boxed::Box;
use dvida_serialize::DvDeserialize;

use crate::{
    crypto::iterators::{Bit, BitIterator},
    drivers::fs::ext2::{BLOCK_SIZE, INODE_SIZE, Inode, structs::Ext2Fs},
    hal::{fs::HalFsOpenErr, storage::SECTOR_SIZE},
};

pub const RESERVED_BOOT_RECORD_OFFSET: i64 = 2;

impl Ext2Fs {
    pub async fn allocated_block(
        &self,
        inode: &mut Inode,
        group_number: i64,
    ) -> Result<(), HalFsOpenErr> {
        if self.super_block.s_prealloc_blocks > 12 {
            todo!("implement prealloc block more than 12");
        }

        let mut remaining_blocks = self.super_block.s_prealloc_blocks;

        let block_group_size =
            self.super_block.s_blocks_per_group as i64 * (BLOCK_SIZE as i64 / SECTOR_SIZE as i64);

        let addr = block_group_size * group_number + RESERVED_BOOT_RECORD_OFFSET + 2;

        let mut buf = Box::new([0u8; BLOCK_SIZE as usize]);

        self.read_sectors(buf.clone(), addr).await?;

        let bit_iterator: BitIterator<u8> = BitIterator::new(buf.as_mut_slice());

        for (idx, bit) in bit_iterator.into_iter().enumerate() {
            if bit == Bit::One {
                continue;
            }

            remaining_blocks -= 1;
        }

        Ok(())
    }

    pub async fn find_available_inode(&self) -> Result<i64, HalFsOpenErr> {
        let block_group_size =
            self.super_block.s_blocks_per_group as i64 * (BLOCK_SIZE as i64 / SECTOR_SIZE as i64);

        let mut inode_count = 0;
        let mut buf = Box::new([0u8; BLOCK_SIZE as usize]);

        for addr in (RESERVED_BOOT_RECORD_OFFSET
            ..RESERVED_BOOT_RECORD_OFFSET + self.entry.end_lba as i64 - self.entry.start_lba as i64
                + 1)
            .step_by(block_group_size as usize)
        {
            let inode_bitmap_loc = addr + 3;
            self.read_sectors(buf.clone(), inode_bitmap_loc).await?;

            let bit_iterator: BitIterator<u8> = BitIterator::<u8> {
                num: buf.as_mut_slice(),
                idx: 0,
                bit_idx: 0,
            };

            for (idx, bit) in bit_iterator.into_iter().enumerate() {
                if inode_count as i64 * INODE_SIZE as i64 + addr + 4
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

                let inode = Inode::deserialize(
                    dvida_serialize::Endianness::Little,
                    &inode_table_buf[idx * INODE_SIZE as usize..],
                )?
                .0;
            }
        }

        Ok(0)
    }

    pub async fn create_file(&mut self, inode: &Inode) -> Result<(), HalFsOpenErr> {
        if !inode.is_directory() {
            return Err(HalFsOpenErr::NotADirectory);
        }

        Ok(())
    }
}
