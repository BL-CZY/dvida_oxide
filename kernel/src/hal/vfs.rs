use crate::{
    crypto::guid::Guid,
    drivers::fs::ext2::structs::Ext2Fs,
    ejcineque::sync::{
        mpsc::unbounded::{UnboundedSender, unbounded_channel},
        spsc::cell::{SpscCellSetter, spsc_cells},
    },
    hal::{gpt::GptReader, storage::get_storage_devices_by_guid},
    log,
};
use alloc::collections::btree_map::BTreeMap;
use once_cell_no_std::OnceCell;

use crate::{
    arch::x86_64::err::ErrNo,
    hal::{
        buffer::Buffer,
        fs::{FileSystem, HalIOCtx, HalInode, OpenFlags},
        path::Path,
    },
};

#[repr(u8)]
pub enum Whence {
    SeekSet = 0,
    SeekCur,
    SeekEnd,
    SeekData,
    SeekHole,
}

pub enum VfsOperationType {
    Open {
        path: Path,
        flags: OpenFlags,
        cell: SpscCellSetter<Result<i64, ErrNo>>,
    },

    Read {
        inode_id: i64,
        buffer: Buffer,
        cell: SpscCellSetter<Result<i64, ErrNo>>,
    },

    Write {
        inode_id: i64,
        buffer: Buffer,
        cell: SpscCellSetter<Result<i64, ErrNo>>,
    },

    Lseek {
        inode_id: i64,
        whence: Whence,
        offset: i64,
        cell: SpscCellSetter<Result<i64, ErrNo>>,
    },

    Close {
        inode_id: i64,
    },
}

pub struct VfsOperation {
    operation_type: VfsOperationType,
}

pub struct HalOpenedInode {
    pub inode: HalInode,
    pub ctx: HalIOCtx,
    pub count: usize,
    pub mount_point_id: i64,
}

impl HalOpenedInode {
    pub fn from_inode(inode: HalInode, id: i64) -> Self {
        Self {
            inode,
            ctx: HalIOCtx::new(),
            count: 1,
            mount_point_id: id,
        }
    }
}

pub static VFS_SENDER: OnceCell<UnboundedSender<VfsOperation>> = OnceCell::new();

#[derive(Default)]
pub struct MountPointArray {
    pub mount_points: BTreeMap<i64, FileSystem>,
    pub path_to_id_map: BTreeMap<Path, i64>,

    pub counter: i64,
}

impl MountPointArray {
    pub fn new() -> Self {
        Self {
            counter: 0,
            ..Default::default()
        }
    }

    pub fn insert(&mut self, path: Path, fs: FileSystem) {
        self.mount_points.insert(self.counter, fs);
        self.path_to_id_map.insert(path, self.counter);
        self.counter += 1;
    }

    pub fn get_mount_point_by_id(&mut self, id: i64) -> Option<&mut FileSystem> {
        self.mount_points.get_mut(&id)
    }

    pub fn get_mount_point_by_path(&mut self, path: &Path) -> Option<&mut FileSystem> {
        match self.path_to_id_map.get(path) {
            Some(id) => self.mount_points.get_mut(id),
            None => None,
        }
    }

    pub fn contains_path(&self, path: &Path) -> bool {
        match self.path_to_id_map.get(path) {
            Some(id) => self.mount_points.get(id).is_some(),
            None => false,
        }
    }
}

macro_rules! find_inode_and_process {
    {  $opened_inodes:ident, $inode_id:ident, $cell:ident, $mount_points:ident, | $inode_alias:ident, $ext2_ino_alias:ident, $ext2_alias:ident | => $ext2_handle:block } => {
        let $inode_alias = match $opened_inodes.get_mut(&$inode_id) {
            Some(inode) => inode,
            None => {
                $cell.set(Err(ErrNo::BadFd));
                continue;
            }
        };

        let fs = match $mount_points.get_mount_point_by_id($inode_alias.mount_point_id) {
            Some(fs) => fs,
            None => {
                $cell.set(Err(ErrNo::BadFd));
                continue;
            }
        };

        match fs.fs_impl {
            crate::hal::fs::HalFs::Ext2(ref mut $ext2_alias) => {
                #[allow(irrefutable_let_patterns)]
                if let HalInode::Ext2(ref mut $ext2_ino_alias) = $inode_alias.inode {

                    $ext2_handle
                } else {
                    $cell.set(Err(ErrNo::BadFd));
                }
            }
            crate::hal::fs::HalFs::Unidentified => panic!("Bad fs"),
        }
    };
}

pub async fn spawn_vfs_task(drive_id: Guid, entry_id: Guid) {
    let (tx, rx) = unbounded_channel::<VfsOperation>();
    let _ = VFS_SENDER.set(tx).expect("Failed to set vfs task sender");

    let mut fs = FileSystem::default();
    let mut opened_inodes: BTreeMap<i64, HalOpenedInode> = BTreeMap::new();
    let mut inode_idx_counter: i64 = 0;
    let mut mount_points = MountPointArray::new();

    let gpt_reader = GptReader::new(
        get_storage_devices_by_guid()
            .lock()
            .await
            .get(&drive_id)
            .expect("Failed to mount root")
            .0,
    );

    let (_header, entries) = gpt_reader.read_gpt().await.expect("Failed to read GPT");
    let entry = {
        let mut res = None;
        for ent in entries.iter() {
            if ent.unique_guid() == entry_id {
                res = Some(ent);
            }
        }
        res.expect("Failed to mount root: cannot find GPT entry")
    };
    log!("Root directory entry: {:?}", entry);

    fs.drive_id = drive_id;
    fs.entry = *entry;
    fs.mounted_at = Path::new_appended("/");

    // only ext2 is supported
    fs.fs_impl = crate::hal::fs::HalFs::Ext2(Ext2Fs::new(drive_id, fs.entry.clone()).await);

    mount_points.insert(Path::new_appended("/"), fs);

    while let Some(operation) = rx.recv().await {
        match operation.operation_type {
            VfsOperationType::Open { path, flags, cell } => {
                let path = path.normalize();

                let (_, id) = mount_points.path_to_id_map.iter().fold(
                    (usize::MAX, None),
                    |(mut acc, mut res), (p, id)| {
                        if path.as_str().starts_with(p.as_str()) && p.as_str().len() < acc {
                            acc = p.as_str().len();
                            res = Some(id);
                        }

                        (acc, res)
                    },
                );

                match id {
                    Some(id) => {
                        let id = *id;
                        let fs = match mount_points.get_mount_point_by_id(id) {
                            Some(fs) => fs,
                            None => {
                                cell.set(Err(ErrNo::NoSuchFileOrDirectory));
                                continue;
                            }
                        };

                        let path = Path::new_appended(
                            path.as_str().trim_start_matches(fs.mounted_at.as_str()),
                        );

                        match fs.fs_impl {
                            crate::hal::fs::HalFs::Ext2(ref mut ext2) => {
                                match ext2.open_file(path, flags).await {
                                    Ok(inode) => {
                                        let inode = HalOpenedInode::from_inode(inode, id);
                                        opened_inodes.insert(inode_idx_counter, inode);
                                        fs.opened_inodes.insert(inode_idx_counter);
                                        cell.set(Ok(inode_idx_counter));
                                        inode_idx_counter += 1;
                                    }
                                    Err(e) => {
                                        cell.set(Err(Into::<ErrNo>::into(e)));
                                    }
                                }
                            }
                            crate::hal::fs::HalFs::Unidentified => panic!("Bad fs"),
                        }
                    }
                    None => {
                        cell.set(Err(ErrNo::NoSuchFileOrDirectory));
                    }
                }
            }

            VfsOperationType::Read {
                inode_id,
                mut buffer,
                cell,
            } => {
                find_inode_and_process!(opened_inodes, inode_id, cell, mount_points, |inode, ino, ext2| => {
                    match ext2.read(ino, &mut buffer, &mut inode.ctx).await {
                        Ok(bytes_read) => {
                            cell.set(Ok(bytes_read as i64));
                        }
                        Err(e) => {
                            cell.set(Err(Into::<ErrNo>::into(e)));
                        }
                    }
                });
            }

            VfsOperationType::Write {
                inode_id,
                buffer,
                cell,
            } => {
                find_inode_and_process!(opened_inodes, inode_id, cell, mount_points, |inode, ino, ext2| => {
                    match ext2.write(ino, &buffer, &mut inode.ctx).await {
                        Ok(bytes_written) => {
                            cell.set(Ok(bytes_written as i64));
                        }
                        Err(e) => {
                            cell.set(Err(Into::<ErrNo>::into(e)));
                        }
                    }

                });
            }

            VfsOperationType::Lseek {
                inode_id,
                whence,
                offset,
                cell,
            } => {
                find_inode_and_process!(opened_inodes, inode_id, cell, mount_points, |inode, _ino, _ext2| => {
                    match whence {
                        Whence::SeekSet => {
                            inode.ctx.head = offset as usize;
                            cell.set(Ok(inode.ctx.head as i64));
                        }
                        Whence::SeekCur => {
                            if offset < 0 {
                                inode.ctx.head -= (offset * -1) as usize;
                            } else {
                                inode.ctx.head += offset as usize;
                            }
                            cell.set(Ok(inode.ctx.head as i64));
                        }
                        Whence::SeekEnd => {}
                        Whence::SeekData => {}
                        Whence::SeekHole => {}
                    }
                });
            }

            VfsOperationType::Close { .. } => {
                todo!();
            }
        }
    }
}

pub async fn vfs_open(path: Path, flags: OpenFlags) -> Result<i64, ErrNo> {
    let sender = VFS_SENDER.get().expect("Failed to get VFS sender");

    let (tx, rx) = spsc_cells::<Result<i64, ErrNo>>();

    sender.send(VfsOperation {
        operation_type: VfsOperationType::Open {
            path,
            flags,
            cell: rx,
        },
    });

    tx.get().await
}

pub async fn vfs_read(fd: i64, buf: Buffer) -> Result<i64, ErrNo> {
    let sender = VFS_SENDER.get().expect("Failed to get VFS sender");

    let (tx, rx) = spsc_cells::<Result<i64, ErrNo>>();

    sender.send(VfsOperation {
        operation_type: VfsOperationType::Read {
            inode_id: fd,
            buffer: buf,
            cell: rx,
        },
    });

    tx.get().await
}

pub async fn vfs_write(fd: i64, buf: Buffer) -> Result<i64, ErrNo> {
    let sender = VFS_SENDER.get().expect("Failed to get VFS sender");

    let (tx, rx) = spsc_cells::<Result<i64, ErrNo>>();

    sender.send(VfsOperation {
        operation_type: VfsOperationType::Write {
            inode_id: fd,
            buffer: buf,
            cell: rx,
        },
    });

    tx.get().await
}

pub async fn vfs_lseek(fd: i64, whence: Whence, offset: i64) -> Result<i64, ErrNo> {
    let sender = VFS_SENDER.get().expect("Failed to get VFS sender");

    let (tx, rx) = spsc_cells::<Result<i64, ErrNo>>();

    sender.send(VfsOperation {
        operation_type: VfsOperationType::Lseek {
            inode_id: fd,
            whence,
            offset,
            cell: rx,
        },
    });

    tx.get().await
}
