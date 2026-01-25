use bitfield::bitfield;
use bytemuck::{Pod, Zeroable};

use crate::drivers::ata::sata::fis::FisRegH2D;

#[repr(C, align(2))]
#[derive(Clone, Copy, Debug)]
pub struct IdentifyData {
    /// Word 0: General configuration (Bit 15: 0=ATA, 1=ATAPI)
    pub config: u16,
    /// Word 1: Logical Cylinders
    pub cylinders: u16,
    pub _reserved_w2: u16,
    /// Word 3: Logical Heads
    pub heads: u16,
    pub _reserved_w4_5: [u16; 2],
    /// Word 6: Logical Sectors per Track
    pub sectors_per_track: u16,
    /// Words 1-9: Obsolete or specific configuration
    pub _reserved1: [u16; 3],
    /// Words 10-19: Serial number (20 ASCII characters, byte-swapped)
    pub serial: [u8; 20],
    /// Words 20-22: Obsolete
    pub _reserved2: [u16; 3],
    /// Words 23-26: Firmware revision (8 ASCII characters, byte-swapped)
    pub firmware_rev: [u8; 8],
    /// Words 27-46: Model number (40 ASCII characters, byte-swapped)
    pub model: [u8; 40],
    /// Word 47: Read/Write multiple support
    pub trusted_computing: u16,
    /// Word 48: Capabilities
    pub capabilities1: u16,
    /// Word 49: Capabilities
    pub capabilities2: u16,
    /// Words 51-59: Obsolete
    pub _reserved3: [u16; 9],
    /// Words 60-61: Total number of user-addressable logical sectors (LBA28)
    pub lba28_sectors: u32,
    /// Words 62-79: Obsolete
    pub _reserved4: [u16; 18],
    /// Word 80: Major version number (Check bits for ACS-x support)
    pub major_version: u16,
    /// Word 81: Minor version number
    pub minor_version: u16,
    /// Word 82: Command set supported
    pub command_set_supported1: u16,
    /// Word 83: Command set supported (Bit 10: 1=LBA48 support)
    pub command_set_supported2: u16,
    /// Words 84-99: Reserved
    pub _reserved5: [u16; 16],
    /// Words 100-103: Total number of user-addressable logical sectors (LBA48)
    pub lba48_sectors: u64,
    /// Words 104-255: Reserved
    pub _reserved6: [u16; 152],
}

bitfield! {
    #[repr(C)]
    pub struct CommandHeaderFlags(u16);
    impl Debug;
    // in DWORDS
    pub cmd_fis_len, set_cmd_fis_len: 4, 0;
    pub is_atapi, set_is_atapi: 5;
    pub is_write, set_is_write: 6;
    pub is_prefetchable, set_is_prefetchable: 7;
    pub reset, set_reset: 8, 8;
    pub bist, set_bist: 9, 9;
    // clears busy as long as an ok handshake is done
    pub clear_busy_when_r_ok, set_clear_busy_when_r_ok: 10;
    pub port_multiplier, set_port_multiplier: 15, 12;
}

#[derive(Pod, Zeroable, Clone, Copy, Default)]
#[repr(C, packed)]
pub struct CommandHeader {
    pub flags: u16,
    /// number of entries
    pub physical_region_descriptor_table_length: u16,
    pub physical_region_descriptor_bytes_count: u32,
    pub cmd_table_base_addr_low: u32,
    pub cmd_table_base_addr_high: u32,
    pub reserved: [u32; 4],
}

#[derive(Pod, Zeroable, Clone, Copy)]
#[repr(C, packed)]
/// the structure of a command table of 0x200 bytes will be:
/// CFIS - 0x40 bytes
/// ACMD - 0x10 bytes
/// reserved - 0x30 bytes
/// prdt - 24 entries, each 16 bytes
pub struct CommandTable {
    pub cmd_fis: FisRegH2D,
    _padding: [u8; 32],
    _padding1: u64,
    _padding2: u32,
    _ata_cmd_area: [u8; 16],
    _reserved: [u8; 0x30],
    pub prdt_table: [PrdtEntry; 24],
}

bitfield! {
    #[repr(C)]
    pub struct PrdtEntryFlags(u32);
    impl Debug;
    // this is byte_len - 1, the controller uses 0..=byte_count
    // has to be an odd number
    pub byte_count, set_byte_count: 21, 0;
    pub interrupt, set_interrupt: 31;
}

#[derive(Pod, Zeroable, Clone, Copy, Default)]
#[repr(C, packed)]
pub struct PrdtEntry {
    // has to be 2 bytes aligned
    pub data_base_low: u32,
    pub data_base_high: u32,
    pub _reserved: u32,
    pub flags: u32,
}
