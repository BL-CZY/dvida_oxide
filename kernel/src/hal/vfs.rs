use terminal::log;

use crate::{
    drivers::fs::ext2::structs::Ext2Fs,
    hal::{fs::FILE_SYSTEM, storage::read_gpt},
};

pub async fn init_vfs(drive_id: usize, entry_idx: usize) {
    let (_header, mut entries) = read_gpt(drive_id).await.expect("Failed to read GPT");
    log!("Root directory entry: {:?}", entries[entry_idx]);

    let mut fs = FILE_SYSTEM.lock().await;

    fs.drive_id = drive_id;
    fs.entry_idx = entry_idx;
    fs.entry = entries.remove(entry_idx);

    // only ext2 is supported
    fs.fs_impl = crate::hal::fs::HalFs::Ext2(Ext2Fs::new(drive_id, fs.entry.clone()).await);
}
