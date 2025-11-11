use alloc::vec::Vec;

use crate::hal::{gpt::GPTEntry, path::Path};

#[derive(Debug, Clone)]
pub struct MountPoint {
    pub fs: FileSystem,
    pub path: Path,
}

#[derive(Debug, Clone)]
pub struct FileSystem {
    pub drive_id: usize,
    pub entry_idx: usize,
    pub entry: GPTEntry,

    pub file_system_type: FileSystemType,
    pub mnt_points: Vec<MountPoint>,
}

#[derive(Debug, Clone)]
pub enum FileSystemType {
    Ext2,
}
