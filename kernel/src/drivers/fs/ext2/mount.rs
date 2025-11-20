use crate::{drivers::fs::ext2::structs::Ext2Fs, hal::gpt::GPTEntry};

impl Ext2Fs {
    pub async fn mount(drive_id: usize, entry: GPTEntry) -> Self {}
}
