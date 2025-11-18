use core::fmt::Debug;

use alloc::{boxed::Box, vec::Vec};

use crate::hal::{gpt::GPTEntry, path::Path};

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
    pub fs_impl: Box<dyn HalFs>,
}

pub trait HalInode {}

pub trait HalFs: Debug {}
