use alloc::boxed::Box;
use terminal::log;

use crate::{
    drivers::fs::ext2::structs::Ext2Fs,
    hal::{
        fs::{FILE_SYSTEM, HalFsIOErr, HalIOCtx, HalInode, OpenFlags},
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

pub async fn write(
    inode: &mut HalInode,
    buf: Box<[u8]>,
    ctx: &mut HalIOCtx,
) -> Result<usize, HalFsIOErr> {
    let mut fs = FILE_SYSTEM.lock().await;

    match fs.fs_impl {
        crate::hal::fs::HalFs::Ext2(ref mut ext2) => {
            let ino = match inode {
                HalInode::Ext2(ino) => ino,
            };
            Ok(ext2.write(ino, buf, ctx).await?)
        }
        super::fs::HalFs::Unidentified => panic!("No file system detected"),
    }
}

pub async fn read(
    inode: &mut HalInode,
    buf: &mut Box<[u8]>,
    ctx: &mut HalIOCtx,
) -> Result<usize, HalFsIOErr> {
    let mut fs = FILE_SYSTEM.lock().await;

    match fs.fs_impl {
        crate::hal::fs::HalFs::Ext2(ref mut ext2) => {
            let ino = match inode {
                HalInode::Ext2(ino) => ino,
            };
            Ok(ext2.read(ino, buf, ctx).await?)
        }
        super::fs::HalFs::Unidentified => panic!("No file system detected"),
    }
}
