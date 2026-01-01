use core::fmt::Debug;

use alloc::{string::String, sync::Arc, vec::Vec};
use dvida_serialize::{DvDeErr, DvSerErr, DvSerialize};
use ejcineque::sync::mutex::Mutex;
use lazy_static::lazy_static;

use crate::{
    drivers::fs::ext2::{self, structs::Ext2Fs},
    hal::{gpt::GPTEntry, path::Path, storage::HalStorageOperationErr},
};

pub const EOF: usize = 0;

lazy_static! {
    pub static ref FILE_SYSTEM: Arc<Mutex<FileSystem>> =
        Arc::new(Mutex::new(FileSystem::default()));
}

pub struct DirEnt64 {
    pub inode_idx: u64,
    pub offset: i64,
    pub file_type: u8,
    pub name: String,
}

impl DirEnt64 {
    pub fn rec_len(&self) -> usize {
        let length = size_of::<u64>()
            + size_of::<i64>()
            + size_of::<u16>()
            + size_of::<u8>()
            + self.name.len()
            + 1;

        let length = (length + 8) & !7;
        length
    }
}

impl DvSerialize for DirEnt64 {
    fn serialize(
        &self,
        endianness: dvida_serialize::Endianness,
        target: &mut [u8],
    ) -> Result<usize, DvSerErr> {
        let length = self.rec_len();

        if target.len() < length {
            return Err(DvSerErr::BufferTooSmall);
        }

        let mut progress = 0;

        progress += self
            .inode_idx
            .serialize(endianness, &mut target[progress..])?;
        progress += self.offset.serialize(endianness, &mut target[progress..])?;
        progress += (length as u16).serialize(endianness, &mut target[progress..])?;
        progress += self
            .file_type
            .serialize(endianness, &mut target[progress..])?;

        for byte in self.name.as_bytes().iter() {
            target[progress] = *byte;
            progress += 1;
        }

        target[progress] = b'\0';

        Ok(length)
    }
}

#[derive(Debug)]
pub struct MountPoint {
    pub fs: FileSystem,
    pub path: Path,
}

#[derive(Debug, Default)]
pub struct FileSystem {
    pub drive_id: usize,
    pub entry_idx: usize,
    pub entry: GPTEntry,

    pub mnt_points: Vec<MountPoint>,
    pub fs_impl: HalFs,
}

#[derive(Debug, Clone, Default)]
pub enum OpenAccessMode {
    #[default]
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

#[derive(Debug, Clone, Default)]
pub struct OpenFlags {
    pub access_mode: OpenAccessMode,
    pub flags: i32,
    pub perms: Option<i32>,
}

#[derive(Debug, Clone)]
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
    DirectoryNotEmpty,
    NoPermsProvided,
    FileTooLarge,
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

#[derive(Debug, Default)]
pub enum HalFs {
    #[default]
    Unidentified,
    Ext2(Ext2Fs),
}
