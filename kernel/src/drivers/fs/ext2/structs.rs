use alloc::vec::Vec;

use crate::{
    drivers::fs::ext2::{GroupDescriptor, SuperBlock},
    hal::gpt::GPTEntry,
};

pub struct Ext2Fs {
    pub drive_id: usize,
    pub entry: GPTEntry,

    pub super_block: SuperBlock,
    pub group_descs: Vec<GroupDescriptor>,
}
