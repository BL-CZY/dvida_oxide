use alloc::{boxed::Box, vec::Vec};

use crate::{
    drivers::fs::ext2::{GroupDescriptor, SuperBlock},
    hal::{
        gpt::GPTEntry,
        storage::{self, HalStorageOperationErr},
    },
};

pub struct Ext2Fs {
    pub drive_id: usize,
    pub entry: GPTEntry,

    pub super_block: SuperBlock,
    pub group_descs: Vec<GroupDescriptor>,
}

impl Ext2Fs {
    pub async fn read_sectors(
        &self,
        buffer: Box<[u8]>,
        lba: i64,
    ) -> Result<(), HalStorageOperationErr> {
        storage::read_sectors(
            self.drive_id,
            &mut buffer,
            self.entry.start_lba as i64 + lba,
        )
        .await?;
    }

    pub async fn write_sectors(
        &self,
        buffer: Box<[u8]>,
        lba: i64,
    ) -> Result<(), HalStorageOperationErr> {
        storage::write_sectors(self.drive_id, &buffer, self.entry.start_lba as i64 + lba).await?;
    }
}
