use alloc::{boxed::Box, vec, vec::Vec};

use crate::{
    crypto::iterators::{Bit, BitIterator},
    drivers::fs::ext2::{
        create_file::AllocatedBlock,
        structs::{BufferManager, GroupManager, IoHandler},
    },
    hal::fs::HalFsIOErr,
};

#[derive(Debug, Clone)]
pub struct BlockAllocator {
    pub block_groups_count: i64,

    pub group_manager: GroupManager,
    pub io_handler: IoHandler,
    pub buffer_manager: BufferManager,
}

impl BlockAllocator {
    pub async fn allocate_n_blocks(
        &self,
        exclude_group_idx: i64,
        mut remaining_blocks: usize,
    ) -> Result<Vec<AllocatedBlock>, HalFsIOErr> {
        let mut blocks_allocated = vec![];
        let group_count = self.block_groups_count;

        // iterate over block groups
        for group_number in 0..(group_count as i64) {
            if group_number == exclude_group_idx {
                continue;
            }

            if remaining_blocks == 0 {
                break;
            }

            let group = self.group_manager.get_group(group_number as i64).await?;
            let mut buf: Box<[u8]> = self.buffer_manager.get_buffer();

            // read block bitmap for the group
            buf = self
                .io_handler
                .read_sectors(buf, group.get_block_bitmap_lba())
                .await?;

            let bit_iterator = BitIterator::new(buf.as_mut());

            for (idx, bit) in bit_iterator.into_iter().enumerate() {
                if remaining_blocks == 0 {
                    break;
                }

                if bit == Bit::One {
                    continue;
                }

                // map bitmap index to data block LBA
                let block_lba = group.get_group_lba() + (idx as i64) * group.sectors_per_block;

                remaining_blocks -= 1;

                blocks_allocated.push(AllocatedBlock {
                    addr: block_lba,
                    block_idx: idx as i64,
                    gr_number: group_number as i64,
                });
            }
        }

        if remaining_blocks > 0 {
            return Err(HalFsIOErr::NoSpaceLeft);
        }

        Ok(blocks_allocated)
    }

    pub async fn allocate_n_blocks_in_group(
        &self,
        group_number: i64,
        mut num: usize,
    ) -> Result<Vec<AllocatedBlock>, HalFsIOErr> {
        let mut blocks_allocated = vec![];
        let group = self.group_manager.get_group(group_number).await?;
        let mut buf: Box<[u8]> = self.buffer_manager.get_buffer();

        // read the block bitmap for the group
        buf = self
            .io_handler
            .read_sectors(buf, group.get_block_bitmap_lba())
            .await?;
        let bit_iterator: BitIterator<u8> = BitIterator::new(buf.as_mut());

        for (idx, bit) in bit_iterator.into_iter().enumerate() {
            if num == 0 {
                break;
            }

            if bit == Bit::One {
                continue;
            }

            let block_lba = group.get_group_lba() + (idx as i64 * group.sectors_per_block);

            num -= 1;
            blocks_allocated.push(AllocatedBlock {
                addr: block_lba,
                block_idx: idx as i64,
                gr_number: group_number,
            });
        }

        if num == 0 {
            return Ok(blocks_allocated);
        }

        // if this group didn't satisfy the request, fall back to scanning entire fs
        blocks_allocated.extend(self.allocate_n_blocks(group_number, num).await?.into_iter());

        Ok(blocks_allocated)
    }
}
