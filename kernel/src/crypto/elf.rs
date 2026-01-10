use bytemuck::{Pod, Zeroable};

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

#[derive(Pod, Zeroable, Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ElfHeader {
    /// magic should be 0x7f 0x45 0x4c 0x46, which stands for 0x7f + ELF
    magic: u32,
    /// 1 = 32 bit, 2 = 64 bit
    bit: u8,
    /// only supports little endian
    encoding: u8,
    header_version: u8,
    /// 0 is for system v
    abi: u8,
    padding: [u8; 8],
    elf_type: u16,
    instruction_set: u16,
    /// currently 1
    version: u32,
    entry_offset: u64,
    header_table_offset: u64,
    section_header_table_offset: u64,
    flags: u32,
    header_size: u16,
    program_header_table_entry_size: u16,
    program_header_table_entry_count: u16,
    section_header_table_entry_size: u16,
    section_header_table_entry_count: u16,
    section_header_string_table_idx: u16,
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
    segment_type: u32,
    flags: u32,
    offset: u64,
    vaddr: u64,
    paddr: u64,
    size_in_file: u64,
    size_in_memory: u64,
    alignment: u64,
}
