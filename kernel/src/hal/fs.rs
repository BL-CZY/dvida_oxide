use alloc::vec::Vec;

use crate::hal::gpt::GPTEntry;

#[derive(Debug, Clone)]
pub struct FileSystem {
    pub drive_id: usize,
    pub entry: GPTEntry,

    pub file_system_type: FileSystemType,
    pub mnt_points: Vec<FileSystem>,
}

#[derive(Debug, Clone)]
pub enum FileSystemType {
    Ext2,
}
