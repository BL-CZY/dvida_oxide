use core::fmt::Debug;

use alloc::vec::Vec;
use dvida_serialize::{DvDeErr, DvSerErr};

use crate::{
    drivers::fs::ext2::{self, structs::Ext2Fs},
    hal::{gpt::GPTEntry, path::Path, storage::HalStorageOperationErr},
};

pub const EOF: usize = 0;

#[derive(Debug)]
pub struct MountPoint {
    pub fs: FileSystem,
    pub path: Path,
}

#[derive(Debug)]
pub struct FileSystem {
    pub drive_id: usize,
    pub entry_idx: usize,
    pub entry: GPTEntry,

    pub mnt_points: Vec<MountPoint>,
    pub fs_impl: HalFs,
}

pub enum OpenAccessMode {
    ReadOnly,
    WriteOnly,
    ReadNWrite,
    Search,
    ExecuteOnly,
}

#[repr(i32)]
pub enum OpenFlagsValue {
    // O_NONBLOCK      do not block on open or for data to become available
    NonBlock = 0x1,
    // O_APPEND        append on each write
    Append = 0x1 << 1,
    // O_CREAT         create file if it does not exist
    CreateIfNotExist = 0x1 << 2,
    // O_TRUNC         truncate size to 0
    Truncate = 0x1 << 3,
    // O_EXCL          error if O_CREAT and the file exists
    ErrorIfCreateFileExists = 0x1 << 4,
    // O_SHLOCK        atomically obtain a shared lock
    SharedLock = 0x1 << 5,
    // O_EXLOCK        atomically obtain an exclusive lock
    ExclusiveLock = 0x1 << 6,
    // O_DIRECTORY     restrict open to a directory
    OpenDirectoryOnly = 0x1 << 7,
    // O_NOFOLLOW      do not follow symlinks
    NoSymlink = 0x1 << 8,
    // O_SYMLINK       allow open of symlinks
    AllowSymlink = 0x1 << 9,
    // O_EVTONLY       descriptor requested for event notifications only
    EventDescriptor = 0x1 << 10,
    // O_CLOEXEC       mark as close-on-exec
    MarkCloseOnExec = 0x1 << 11,
    // O_NOFOLLOW_ANY  do not follow symlinks in the entire path
    NoSymlinkAny = 0x1 << 12,
    // O_RESOLVE_BENEATH       path resolution must not escape the directory associated with the file descriptor
    ResolveBeneath = 0x1 << 13,
    // O_UNIQUE        ensure a file is opened only if it has a single hard link
    Unique = 0x1 << 14,
}

pub struct OpenFlags {
    pub access_mode: OpenAccessMode,
    pub flags: i32,
    pub perms: i32,
}

pub enum HalInode {
    Ext2(ext2::InodePlus),
}

#[derive(Debug)]
pub enum HalFsMountErr {}

#[derive(Debug)]
pub enum HalFsIOErr {
    HalErr(HalStorageOperationErr),
    DeserializationErr(DvDeErr),
    SerializationErr(DvSerErr),
    BadPath,
    NameTooLong,
    BufTooSmall,
    IsDirectory,
    Internal,
    NoSpaceLeft,
    NoSuchFileOrDirectory,
    NotADirectory,
    NoAvailableInode,
    FileExists,
    Unsupported,
}

#[derive(Debug)]
pub struct HalIOCtx {
    pub head: usize,
}

impl From<DvDeErr> for HalFsIOErr {
    fn from(value: DvDeErr) -> Self {
        Self::DeserializationErr(value)
    }
}

impl From<DvSerErr> for HalFsIOErr {
    fn from(value: DvSerErr) -> Self {
        Self::SerializationErr(value)
    }
}

impl From<HalStorageOperationErr> for HalFsIOErr {
    fn from(value: HalStorageOperationErr) -> Self {
        Self::HalErr(value)
    }
}

#[derive(Debug)]
pub enum HalFs {
    Ext2(Ext2Fs),
}
