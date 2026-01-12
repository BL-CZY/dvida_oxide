use alloc::{boxed::Box, vec, vec::Vec};
use bytemuck::{Pod, Zeroable};

use crate::{
    arch::x86_64::err::ErrNo,
    hal::{
        buffer::Buffer,
        vfs::{vfs_lseek, vfs_read},
    },
};

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Encoding {
    Unknown = 0,
    LittleEndian = 1,
    BigEndian = 2,
}

#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub enum ElfType {
    Unknown = 0,
    Relocatable = 1,
    Executable = 2,
    Shared = 3,
    Core = 4,
}

pub const ELF_MAGIC: [u8; 4] = [0x7f, 0x45, 0x4c, 0x46];

pub const LONG_BIT: u8 = 2;
pub const SYSTEM_V: u8 = 0;

#[derive(Pod, Zeroable, Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ElfHeader {
    /// magic should be 0x7f 0x45 0x4c 0x46, which stands for 0x7f + ELF
    pub magic: [u8; 4],
    /// 1 = 32 bit, 2 = 64 bit
    pub bit: u8,
    /// only supports little endian
    pub encoding: u8,
    pub header_version: u8,
    /// 0 is for system v
    pub abi: u8,
    pub padding: [u8; 8],
    pub elf_type: u16,
    pub instruction_set: u16,
    /// currently 1
    pub version: u32,
    pub entry_offset: u64,
    pub header_table_offset: u64,
    pub section_header_table_offset: u64,
    pub flags: u32,
    pub header_size: u16,
    pub program_header_table_entry_size: u16,
    pub program_header_table_entry_count: u16,
    pub section_header_table_entry_size: u16,
    pub section_header_table_entry_count: u16,
    pub section_header_string_table_idx: u16,
}

#[derive(Debug, Clone)]
pub struct ElfFile {
    pub header: ElfHeader,
    pub program_header_table: Vec<ElfProgramHeaderEntry>,
    pub section_header_table: Vec<ELFSectionHeaderEntry>,
}

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum Flags {
    Executable = 0b1,
    Writable = 0b10,
    Readable = 0b100,
}

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum SegmentType {
    Null = 0,
    Load = 1,
    Dynamic = 2,
    Interp = 3,
    Note = 4,
}

#[derive(Pod, Zeroable, Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ElfProgramHeaderEntry {
    pub segment_type: u32,
    pub flags: u32,
    /// offset in file
    pub offset: u64,
    pub vaddr: u64,
    pub paddr: u64,
    pub size_in_file: u64,
    pub size_in_memory: u64,
    pub alignment: u64,
}

#[derive(Pod, Zeroable, Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ELFSectionHeaderEntry {
    /// offset in the .shstrtab section that contains the name
    pub name_offset: u32,
    pub section_type: u32,
    pub flags: u64,
    pub addr: u64,
    pub offset: u64,
    pub size: u64,
    pub link: u32,
    pub info: u32,
    pub addralign: u64,
    pub entry_size: u64,
}

#[derive(Debug)]
pub enum ElfErr {
    FsErr(ErrNo),
    NotELF,
    Unsupported,
    Corrupted,
}

impl From<ErrNo> for ElfErr {
    fn from(value: ErrNo) -> Self {
        Self::FsErr(value)
    }
}

async fn read_elf_header(fd: i64) -> Result<ElfHeader, ElfErr> {
    const BUF_SIZE: usize = 1024;

    let buf = vec![0u8; BUF_SIZE].into_boxed_slice();
    let buf: Buffer = buf.into();

    let bytes_read = vfs_read(fd, buf.clone()).await?;

    if bytes_read < size_of::<ElfHeader>() as i64 {
        return Err(ElfErr::NotELF);
    }

    let elf_header: ElfHeader = *bytemuck::from_bytes(&buf[0..size_of::<ElfHeader>()]);

    if elf_header.magic != ELF_MAGIC {
        return Err(ElfErr::NotELF);
    }

    if elf_header.bit != LONG_BIT
        || elf_header.abi != SYSTEM_V
        || elf_header.encoding != Encoding::LittleEndian as u8
    {
        return Err(ElfErr::Unsupported);
    }

    let buf: Box<[u8]> = buf.into();
    drop(buf);

    Ok(elf_header)
}

pub async fn read_program_headers(
    elf_header: &ElfHeader,
    fd: i64,
) -> Result<Vec<ElfProgramHeaderEntry>, ElfErr> {
    vfs_lseek(
        fd,
        crate::hal::vfs::Whence::SeekSet,
        elf_header.header_table_offset as i64,
    )
    .await?;

    let entry_table_size =
        elf_header.program_header_table_entry_size * elf_header.program_header_table_entry_count;

    let buf = vec![0u8; entry_table_size.into()].into_boxed_slice();
    let buf: Buffer = buf.into();

    let bytes_read = vfs_read(fd, buf.clone()).await?;

    if bytes_read < entry_table_size as i64 {
        return Err(ElfErr::NotELF);
    }

    let mut programs_headers: Vec<ElfProgramHeaderEntry> = vec![];
    for i in 0..elf_header.program_header_table_entry_count {
        let offset = i * elf_header.program_header_table_entry_size;
        let offset = offset as usize;
        let entry: ElfProgramHeaderEntry =
            *bytemuck::from_bytes(&buf[offset..offset + size_of::<ElfProgramHeaderEntry>()]);

        if entry.size_in_memory < entry.size_in_file {
            return Err(ElfErr::Corrupted);
        }

        programs_headers.push(entry);
    }

    let buf: Box<[u8]> = buf.into();
    drop(buf);

    Ok(programs_headers)
}

pub async fn read_section_headers(
    elf_header: &ElfHeader,
    fd: i64,
) -> Result<Vec<ELFSectionHeaderEntry>, ElfErr> {
    vfs_lseek(
        fd,
        crate::hal::vfs::Whence::SeekSet,
        elf_header.section_header_table_offset as i64,
    )
    .await?;

    let section_table_size =
        elf_header.section_header_table_entry_size * elf_header.section_header_table_entry_count;

    let buf = vec![0u8; section_table_size.into()].into_boxed_slice();
    let buf: Buffer = buf.into();

    let bytes_read = vfs_read(fd, buf.clone()).await?;

    if bytes_read < section_table_size as i64 {
        return Err(ElfErr::NotELF);
    }

    let mut programs_headers: Vec<ELFSectionHeaderEntry> = vec![];
    for i in 0..elf_header.section_header_table_entry_count {
        let offset = i * elf_header.section_header_table_entry_size;
        let offset = offset as usize;
        let entry: ELFSectionHeaderEntry =
            *bytemuck::from_bytes(&buf[offset..offset + size_of::<ELFSectionHeaderEntry>()]);
        programs_headers.push(entry);
    }

    let buf: Box<[u8]> = buf.into();
    drop(buf);

    Ok(programs_headers)
}

pub async fn read_elf(fd: i64) -> Result<ElfFile, ElfErr> {
    let elf_header = read_elf_header(fd).await?;
    let program_headers = read_program_headers(&elf_header, fd).await?;
    let section_headers = read_section_headers(&elf_header, fd).await?;

    Ok(ElfFile {
        header: elf_header,
        program_header_table: program_headers,
        section_header_table: section_headers,
    })
}
