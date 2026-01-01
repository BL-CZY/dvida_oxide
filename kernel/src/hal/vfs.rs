use terminal::log;

use crate::{
    drivers::fs::ext2::structs::Ext2Fs,
    hal::{
        fs::{FILE_SYSTEM, HalFsIOErr, HalInode, OpenFlags},
        path::Path,
        storage::read_gpt,
    },
};

#[repr(i8)]
pub enum OpenErr {
    NoSuchFileOrDirectory = -2,
    PermissionDenied = -13,
    FileExists = -17,
    IsADirectory = -21,
    TooManyOpenFiles = -24,
    Unknown = -128,
}

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

pub async fn open(path: Path, flags: OpenFlags) -> Result<HalInode, HalFsIOErr> {
    let mut fs = FILE_SYSTEM.lock().await;

    match fs.fs_impl {
        crate::hal::fs::HalFs::Ext2(ref mut ext2) => Ok(ext2.open_file(path, flags).await?),
        super::fs::HalFs::Unidentified => panic!("No file system detected"),
    }
}
