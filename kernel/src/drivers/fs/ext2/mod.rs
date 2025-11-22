pub mod init;
pub mod mount;
pub mod open;
pub mod structs;

use alloc::string::String;
use dvida_serialize::*;

/// The ext2 superblock structure - located at byte offset 1024 from start
/// All fields stored in little-endian format on disk
#[derive(DvDeSer, Debug, Clone)]
pub struct SuperBlock {
    // Base fields (revision 0 and 1)
    s_inodes_count: u32,      // Total number of inodes
    s_blocks_count: u32,      // Total number of blocks
    s_r_blocks_count: u32,    // Number of reserved blocks
    s_free_blocks_count: u32, // Number of free blocks
    s_free_inodes_count: u32, // Number of free inodes
    s_first_data_block: u32,  // First data block (0 or 1)
    s_log_block_size: u32,    // Block size = 1024 << s_log_block_size
    s_log_frag_size: u32,     // Fragment size = 1024 << s_log_frag_size
    s_blocks_per_group: u32,  // Number of blocks per group
    s_frags_per_group: u32,   // Number of fragments per group
    s_inodes_per_group: u32,  // Number of inodes per group
    s_mtime: u32,             // Last mount time (POSIX time)
    s_wtime: u32,             // Last write time (POSIX time)
    s_mnt_count: u16,         // Mount count since last fsck
    s_max_mnt_count: u16,     // Maximum mount count before fsck
    s_magic: u16,             // Magic signature (0xEF53)
    s_state: u16,             // File system state
    s_errors: u16,            // Behavior when detecting errors
    s_minor_rev_level: u16,   // Minor revision level
    s_lastcheck: u32,         // Last check time (POSIX time)
    s_checkinterval: u32,     // Maximum time between checks
    s_creator_os: u32,        // OS that created the filesystem
    s_rev_level: u32,         // Revision level
    s_def_resuid: u16,        // Default user ID for reserved blocks
    s_def_resgid: u16,        // Default group ID for reserved blocks

    // Extended fields (EXT2_DYNAMIC_REV - revision 1)
    s_first_ino: u32,         // First non-reserved inode
    s_inode_size: u16,        // Size of inode structure (bytes)
    s_block_group_nr: u16,    // Block group number of this superblock
    s_feature_compat: u32,    // Compatible feature set
    s_feature_incompat: u32,  // Incompatible feature set
    s_feature_ro_compat: u32, // Read-only compatible feature set
    s_uuid: [u8; 16],         // 128-bit filesystem UUID
    s_volume_name: [u8; 16],  // Volume name (null-terminated)
    s_last_mounted: [u8; 64], // Directory where last mounted
    s_algo_bitmap: u32,       // Compression algorithms used

    // Performance hints
    s_prealloc_blocks: u8,     // Number of blocks to preallocate for files
    s_prealloc_dir_blocks: u8, // Number of blocks to preallocate for dirs
    s_padding1: u16,           // Alignment padding

    // Journaling support (ext3)
    s_journal_uuid: [u8; 16], // UUID of journal superblock
    s_journal_inum: u32,      // Inode number of journal file
    s_journal_dev: u32,       // Device number of journal file
    s_last_orphan: u32,       // Start of list of orphaned inodes

    // Directory indexing support (HTREE)
    s_hash_seed: [u32; 4],  // Seeds used for hash algorithm
    s_def_hash_version: u8, // Default hash version
    reserved: [u8; 3],

    // Default mount options
    s_default_mount_opts: u32, // Default mount options
    s_first_meta_bg: u32,      // First metablock block group

                               // reserved: [u8; 760],
}

/// Block Group Descriptor structure
#[derive(DvDeSer, Debug, Clone)]
pub struct GroupDescriptor {
    bg_block_bitmap: u32,      // Block number of block bitmap
    bg_inode_bitmap: u32,      // Block number of inode bitmap
    bg_inode_table: u32,       // Block number of inode table
    bg_free_blocks_count: u16, // Number of free blocks
    bg_free_inodes_count: u16, // Number of free inodes
    bg_used_dirs_count: u16,   // Number of directories
}

/// Inode structure - represents a file, directory, or other filesystem object
#[derive(DvDeSer, Debug, Clone, Default)]
pub struct Inode {
    i_mode: u16,        // File mode (type and permissions)
    i_uid: u16,         // Low 16 bits of owner UID
    i_size: u32,        // Size in bytes
    i_atime: u32,       // Access time
    i_ctime: u32,       // Creation time
    i_mtime: u32,       // Modification time
    i_dtime: u32,       // Deletion time
    i_gid: u16,         // Low 16 bits of group ID
    i_links_count: u16, // Links count
    i_blocks: u32,      // Blocks count (512-byte blocks)
    i_flags: u32,       // File flags
    i_osd1: u32,        // OS dependent field 1
    i_block: [u32; 15], // Pointers to blocks
    i_generation: u32,  // File version (for NFS)
    i_file_acl: u32,    // File ACL (extended attributes)
    i_dir_acl: u32,     // Directory ACL (or high 32 bits of size)
    i_faddr: u32,       // Fragment address
    i_osd2: [u8; 12],   // OS dependent field 2
}

/// Directory entry structure (variable length)
/// The name_len field is abstracted away for the name field
/// For serialization and deserialization, it will return the next accumulator to where the next entry is
#[derive(Debug, Clone)]
pub struct DirEntry {
    inode: u32,   // Inode number (0 if entry is unused)
    rec_len: u16, // Distance to next directory entry
    // name_len: u8,  // Name length
    file_type: u8, // File type
    name: String,  // File name (variable length, not null-terminated)
}

impl DvDeserialize for DirEntry {
    fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
    where
        Self: Sized,
    {
        let mut acc: usize = 0;

        let (inode, size) = u32::deserialize(endianness, &input[acc..])?;
        acc += size;
        let (rec_len, size) = u16::deserialize(endianness, &input[acc..])?;
        acc += size;
        let (name_len, size) = u8::deserialize(endianness, &input[acc..])?;
        acc += size;
        let (file_type, size) = u8::deserialize(endianness, &input[acc..])?;
        acc += size;

        let mut name = String::new();
        for i in 0..name_len as usize {
            if i >= input[acc..].len() {
                return Err(DvDeErr::WrongBufferSize);
            }

            name.push(input[acc..][i] as char);
        }

        // set acc to be rec_len so it points to the next entry
        acc = rec_len as usize;
        if acc >= input.len() {
            return Err(DvDeErr::WrongBufferSize);
        }

        Ok((
            DirEntry {
                inode,
                rec_len,
                file_type,
                name,
            },
            acc,
        ))
    }
}

impl DvSerialize for DirEntry {
    fn serialize(&self, endianness: Endianness, target: &mut [u8]) -> Result<usize, DvSerErr> {
        let mut acc: usize = 0;

        if self.name.len() > 255 {
            return Err(DvSerErr::BadStringLength(0, 255));
        }

        let name_len = self.name.len() as u8;

        acc += self.inode.serialize(endianness, &mut target[acc..])?;
        acc += self.rec_len.serialize(endianness, &mut target[acc..])?;
        acc += name_len.serialize(endianness, &mut target[acc..])?; // name_len is ignored here 
        acc += self.file_type.serialize(endianness, &mut target[acc..])?;

        for (idx, char) in self.name.bytes().enumerate() {
            target[acc..][idx] = char;
        }

        acc = self.rec_len as usize;
        Ok(acc)
    }
}

pub const BLOCK_SIZE: u32 = 1024;
pub const LOG_BLOCK_SIZE: u32 = 1;
pub const S_R_BLOCKS_COUNT: u32 = 1024;
pub const FIRST_DATA_BLOCK: u32 = 1;
pub const MAX_MOUNT_COUNT: u16 = 64;
pub const CREATOR_OS_DVIDA: u32 = 5;
pub const ROOT_ID: u16 = 0;
pub const ALGO_BITMAP: u32 = 2;
pub const BLOCKS_PER_GROUP: u32 = BLOCK_SIZE * 8;
pub const INODES_PER_GROUP: u32 = BLOCKS_PER_GROUP;
pub const INODE_SIZE: i64 = 32;

// Filesystem state values for s_state
pub const EXT2_VALID_FS: u16 = 0x0001; // Unmounted cleanly
pub const EXT2_ERROR_FS: u16 = 0x0002; // Errors detected

// Error handling methods for s_errors
pub const EXT2_ERRORS_CONTINUE: u16 = 1; // Continue execution
pub const EXT2_ERRORS_RO: u16 = 2; // Remount read-only
pub const EXT2_ERRORS_PANIC: u16 = 3; // Cause a kernel panic

// Creator OS values for s_creator_os
pub const EXT2_OS_LINUX: u32 = 0;
pub const EXT2_OS_HURD: u32 = 1;
pub const EXT2_OS_MASIX: u32 = 2;
pub const EXT2_OS_FREEBSD: u32 = 3;
pub const EXT2_OS_LITES: u32 = 4;

// Revision levels for s_rev_level
pub const EXT2_GOOD_OLD_REV: u32 = 0; // Original format
pub const EXT2_DYNAMIC_REV: u32 = 1; // V2 format with dynamic inode sizes

// Magic number
pub const EXT2_SUPER_MAGIC: u16 = 0xEF53;

// Compatible features (s_feature_compat)
pub const EXT2_FEATURE_COMPAT_DIR_PREALLOC: u32 = 0x0001;
pub const EXT2_FEATURE_COMPAT_IMAGIC_INODES: u32 = 0x0002;
pub const EXT2_FEATURE_COMPAT_HAS_JOURNAL: u32 = 0x0004; // ext3
pub const EXT2_FEATURE_COMPAT_EXT_ATTR: u32 = 0x0008;
pub const EXT2_FEATURE_COMPAT_RESIZE_INODE: u32 = 0x0010;
pub const EXT2_FEATURE_COMPAT_DIR_INDEX: u32 = 0x0020;

// Incompatible features (s_feature_incompat)
pub const EXT2_FEATURE_INCOMPAT_COMPRESSION: u32 = 0x0001;
pub const EXT2_FEATURE_INCOMPAT_FILETYPE: u32 = 0x0002;
pub const EXT2_FEATURE_INCOMPAT_RECOVER: u32 = 0x0004;
pub const EXT2_FEATURE_INCOMPAT_JOURNAL_DEV: u32 = 0x0008;
pub const EXT2_FEATURE_INCOMPAT_META_BG: u32 = 0x0010;

// Read-only compatible features (s_feature_ro_compat)
pub const EXT2_FEATURE_RO_COMPAT_SPARSE_SUPER: u32 = 0x0001;
pub const EXT2_FEATURE_RO_COMPAT_LARGE_FILE: u32 = 0x0002;
pub const EXT2_FEATURE_RO_COMPAT_BTREE_DIR: u32 = 0x0004;

// Inode flags (i_flags)
pub const EXT2_SECRM_FL: u32 = 0x00000001; // Secure deletion
pub const EXT2_UNRM_FL: u32 = 0x00000002; // Undelete
pub const EXT2_COMPR_FL: u32 = 0x00000004; // Compress file
pub const EXT2_SYNC_FL: u32 = 0x00000008; // Synchronous updates
pub const EXT2_IMMUTABLE_FL: u32 = 0x00000010; // Immutable file
pub const EXT2_APPEND_FL: u32 = 0x00000020; // Append only
pub const EXT2_NODUMP_FL: u32 = 0x00000040; // Do not dump file
pub const EXT2_NOATIME_FL: u32 = 0x00000080; // Do not update atime

// File type values for directory entries (file_type field)
pub const EXT2_FT_UNKNOWN: u8 = 0;
pub const EXT2_FT_REG_FILE: u8 = 1;
pub const EXT2_FT_DIR: u8 = 2;
pub const EXT2_FT_CHRDEV: u8 = 3;
pub const EXT2_FT_BLKDEV: u8 = 4;
pub const EXT2_FT_FIFO: u8 = 5;
pub const EXT2_FT_SOCK: u8 = 6;
pub const EXT2_FT_SYMLINK: u8 = 7;

// File mode bits (i_mode)
pub const EXT2_S_IFSOCK: u16 = 0xC000; // Socket
pub const EXT2_S_IFLNK: u16 = 0xA000; // Symbolic link
pub const EXT2_S_IFREG: u16 = 0x8000; // Regular file
pub const EXT2_S_IFBLK: u16 = 0x6000; // Block device
pub const EXT2_S_IFDIR: u16 = 0x4000; // Directory
pub const EXT2_S_IFCHR: u16 = 0x2000; // Character device
pub const EXT2_S_IFIFO: u16 = 0x1000; // FIFO

// Permission bits
pub const EXT2_S_ISUID: u16 = 0x0800; // SUID
pub const EXT2_S_ISGID: u16 = 0x0400; // SGID
pub const EXT2_S_ISVTX: u16 = 0x0200; // Sticky bit
pub const EXT2_S_IRUSR: u16 = 0x0100; // User read
pub const EXT2_S_IWUSR: u16 = 0x0080; // User write
pub const EXT2_S_IXUSR: u16 = 0x0040; // User execute
pub const EXT2_S_IRGRP: u16 = 0x0020; // Group read
pub const EXT2_S_IWGRP: u16 = 0x0010; // Group write
pub const EXT2_S_IXGRP: u16 = 0x0008; // Group execute
pub const EXT2_S_IROTH: u16 = 0x0004; // Others read
pub const EXT2_S_IWOTH: u16 = 0x0002; // Others write
pub const EXT2_S_IXOTH: u16 = 0x0001; // Others execute

// Reserved inode numbers
pub const EXT2_BAD_INO: u32 = 1; // Bad blocks inode
pub const EXT2_ROOT_INO: u32 = 2; // Root directory inode
pub const EXT2_ACL_IDX_INO: u32 = 3; // ACL index inode
pub const EXT2_ACL_DATA_INO: u32 = 4; // ACL data inode
pub const EXT2_BOOT_LOADER_INO: u32 = 5; // Boot loader inode
pub const EXT2_UNDEL_DIR_INO: u32 = 6; // Undelete directory inode

impl SuperBlock {
    /// Returns the actual block size in bytes
    pub fn block_size(&self) -> u32 {
        1024 << self.s_log_block_size
    }

    /// Returns the actual fragment size in bytes
    pub fn fragment_size(&self) -> u32 {
        1024 << self.s_log_frag_size
    }

    /// Checks if the superblock has a valid magic number
    pub fn is_valid(&self) -> bool {
        self.s_magic == EXT2_SUPER_MAGIC
    }

    /// Returns the total number of block groups
    pub fn block_groups_count(&self) -> u32 {
        (self.s_blocks_count + self.s_blocks_per_group - 1) / self.s_blocks_per_group
    }

    /// Returns true if this is a dynamic revision filesystem
    pub fn is_dynamic_rev(&self) -> bool {
        self.s_rev_level >= EXT2_DYNAMIC_REV
    }
}

impl Inode {
    /// Returns the file type from the mode field
    pub fn file_type(&self) -> u16 {
        self.i_mode & 0xF000
    }

    /// Returns the permissions from the mode field
    pub fn permissions(&self) -> u16 {
        self.i_mode & 0x0FFF
    }

    /// Checks if this is a directory
    pub fn is_directory(&self) -> bool {
        self.file_type() == EXT2_S_IFDIR
    }

    /// Checks if this is a regular file
    pub fn is_regular_file(&self) -> bool {
        self.file_type() == EXT2_S_IFREG
    }

    /// Checks if this is a symbolic link
    pub fn is_symlink(&self) -> bool {
        self.file_type() == EXT2_S_IFLNK
    }
}
