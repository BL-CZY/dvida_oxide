use crate::ejcineque::sync::mutex::Mutex;
use alloc::{
    boxed::Box,
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    sync::Arc,
    vec,
    vec::Vec,
};

use crate::{
    crypto::iterators::{Bit, BitIterator},
    drivers::fs::ext2::{
        BLOCK_GROUP_DESCRIPTOR_SIZE, GroupDescriptor,
        create_file::AllocatedBlock,
        structs::{BufferManager, GroupManager, IoHandler},
    },
    hal::{fs::HalFsIOErr, storage::SECTOR_SIZE},
};

#[derive(Debug, Clone)]
pub struct BlockAllocator {
    pub block_groups_count: i64,

    pub group_manager: GroupManager,
    pub io_handler: IoHandler,
    pub buffer_manager: BufferManager,

    pub allocated_block_indices: Arc<Mutex<BTreeSet<AllocatedBlock>>>,
    pub unwritten_freed_blocks: Arc<Mutex<BTreeSet<u32>>>,
}

impl BlockAllocator {
    pub async fn allocate_n_blocks(
        &mut self,
        exclude_group_idx: i64,
        mut remaining_blocks: usize,
    ) -> Result<Vec<AllocatedBlock>, HalFsIOErr> {
        let mut blocks_allocated = vec![];
        let group_count = self.block_groups_count;

        // iterate over block groups
        for group_number in 0..group_count {
            if group_number == exclude_group_idx {
                continue;
            }

            if remaining_blocks == 0 {
                break;
            }

            let group = self.group_manager.get_group(group_number).await?;
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

                let global_idx =
                    group_number as u32 * self.group_manager.blocks_per_group + idx as u32;

                let allocated_block = AllocatedBlock {
                    addr: block_lba,
                    block_relatve_idx: idx as u32,
                    gr_number: group_number,
                    block_global_idx: global_idx,
                };

                if self
                    .allocated_block_indices
                    .lock()
                    .await
                    .contains(&allocated_block)
                {
                    continue;
                }

                remaining_blocks -= 1;

                self.allocated_block_indices
                    .lock()
                    .await
                    .insert(allocated_block.clone());

                blocks_allocated.push(allocated_block);
            }
        }

        if remaining_blocks > 0 {
            return Err(HalFsIOErr::NoSpaceLeft);
        }

        Ok(blocks_allocated)
    }

    pub async fn allocate_n_blocks_in_group(
        &mut self,
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

            // map bitmap index to data block LBA
            let block_lba = group.get_group_lba() + (idx as i64) * group.sectors_per_block;

            let global_idx =
                group_number as u32 * self.group_manager.blocks_per_group + idx as u32;

            let allocated_block = AllocatedBlock {
                addr: block_lba,
                block_relatve_idx: idx as u32,
                gr_number: group_number,
                block_global_idx: global_idx,
            };

            if self
                .allocated_block_indices
                .lock()
                .await
                .contains(&allocated_block)
            {
                continue;
            }

            num -= 1;

            self.allocated_block_indices
                .lock()
                .await
                .insert(allocated_block.clone());

            blocks_allocated.push(allocated_block);
        }

        if num == 0 {
            return Ok(blocks_allocated);
        }

        // if this group didn't satisfy the request, fall back to scanning entire fs
        blocks_allocated.extend(self.allocate_n_blocks(group_number, num).await?.into_iter());

        Ok(blocks_allocated)
    }

    pub async fn write_newly_allocated_blocks(
        &mut self,
        mut buf: Box<[u8]>,
    ) -> Result<(), HalFsIOErr> {
        let mut cur_bitmap_lba = -1;

        let mut allocated_blocks_map: BTreeMap<i64, i64> = BTreeMap::new();

        for AllocatedBlock {
            gr_number,
            block_global_idx: block_idx,
            ..
        } in self.allocated_block_indices.lock().await.iter()
        {
            allocated_blocks_map
                .entry(*gr_number)
                .and_modify(|v| *v += 1)
                .or_insert(1);

            let group = self.group_manager.get_group(*gr_number).await?;
            let block_bitmap_lba = group.get_block_bitmap_lba();

            if cur_bitmap_lba != block_bitmap_lba {
                if cur_bitmap_lba != -1 {
                    self.io_handler
                        .write_sectors(buf.clone(), cur_bitmap_lba)
                        .await?;
                }

                buf = self.io_handler.read_sectors(buf, block_bitmap_lba).await?;
                cur_bitmap_lba = block_bitmap_lba
            }

            let mut target = buf[*block_idx as usize / 8];
            target |= 0x1 << (*block_idx as usize % 8);
            buf[*block_idx as usize / 8] = target;
        }

        self.io_handler
            .write_sectors(buf.clone(), cur_bitmap_lba)
            .await?;

        let mut cur_group_buffer_lba = -1;
        for (group_idx, num_allocated) in allocated_blocks_map {
            let bg_table_block_idx = self.group_manager.first_data_block + 1;
            let lba = self.io_handler.block_idx_to_lba(bg_table_block_idx);
            let lba_offset = (group_idx * BLOCK_GROUP_DESCRIPTOR_SIZE as i64) / SECTOR_SIZE as i64;
            let byte_offset = (group_idx * BLOCK_GROUP_DESCRIPTOR_SIZE as i64) % SECTOR_SIZE as i64;

            if lba + lba_offset != cur_group_buffer_lba {
                if cur_group_buffer_lba != -1 {
                    self.io_handler
                        .write_sectors(buf.clone(), cur_group_buffer_lba)
                        .await?;
                }

                cur_group_buffer_lba = lba + lba_offset;
                buf = self.io_handler.read_sectors(buf, lba + lba_offset).await?;
            }

            let descriptor: &mut GroupDescriptor = bytemuck::from_bytes_mut(
                &mut buf[byte_offset as usize..byte_offset as usize + size_of::<GroupDescriptor>()],
            );

            descriptor.bg_free_blocks_count -= num_allocated as u16;
        }

        self.io_handler
            .write_sectors(buf, cur_group_buffer_lba)
            .await?;

        self.allocated_block_indices.lock().await.clear();

        Ok(())
    }

    pub async fn add_freed_block(&mut self, block: u32) {
        self.unwritten_freed_blocks.lock().await.insert(block);
    }

    pub async fn write_freed_blocks(&mut self) -> Result<(), HalFsIOErr> {
        let mut buf = self.buffer_manager.get_buffer();
        let mut cur_group_buffer_lba = -1;
        for group_idx in self
            .unwritten_freed_blocks
            .lock()
            .await
            .iter()
            .map(|e| *e as i64 / self.group_manager.blocks_per_group as i64)
        {
            let bg_table_block_idx = self.group_manager.first_data_block + 1;
            let lba = self.io_handler.block_idx_to_lba(bg_table_block_idx);
            let lba_offset = (group_idx * BLOCK_GROUP_DESCRIPTOR_SIZE as i64) / SECTOR_SIZE as i64;
            let byte_offset = (group_idx * BLOCK_GROUP_DESCRIPTOR_SIZE as i64) % SECTOR_SIZE as i64;

            if lba + lba_offset != cur_group_buffer_lba {
                if cur_group_buffer_lba != -1 {
                    self.io_handler
                        .write_sectors(buf.clone(), cur_group_buffer_lba)
                        .await?;
                }

                cur_group_buffer_lba = lba + lba_offset;
                buf = self.io_handler.read_sectors(buf, lba + lba_offset).await?;
            }

            let descriptor: &mut GroupDescriptor = bytemuck::from_bytes_mut(
                &mut buf[byte_offset as usize..byte_offset as usize + size_of::<GroupDescriptor>()],
            );

            descriptor.bg_free_blocks_count -= 1;
        }

        self.unwritten_freed_blocks.lock().await.clear();

        Ok(())
    }
}
