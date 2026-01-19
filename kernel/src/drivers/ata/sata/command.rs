use bitfield::bitfield;

use crate::drivers::ata::sata::fis::FisRegH2D;

bitfield! {
    #[repr(C)]
    pub struct CommandHeaderFlags(u16);
    impl Debug;
    // in DWORDS
    pub cmd_fis_len, set_cmd_fis_len: 4, 0;
    pub atapi, set_atapi: 5, 5;
    pub write, set_write: 6, 6;
    pub prefetchable, set_prefetchable: 7, 7;
    pub reset, set_reset: 8, 8;
    pub bist, set_bist: 9, 9;
    pub clear_busy_when_r_ok, set_clear_busy_when_r_ok: 10, 10;
    pub port_multiplier, set_port_multiplier: 15, 12;
}

#[repr(C, packed)]
pub struct CommandHeader {
    pub flags: CommandHeaderFlags,
    /// number of entries
    pub physical_region_descriptor_table_length: u16,
    pub physical_region_descriptor_bytes_count: u32,
    pub cmd_table_base_addr_low: u32,
    pub cmd_table_base_addr_high: u32,
    pub reserved: [u32; 4],
}

#[repr(C, packed)]
/// the structure of a command table of 0x200 bytes will be:
/// CFIS - 0x40 bytes
/// ACMD - 0x10 bytes
/// reserved - 0x30 bytes
/// prdt - 24 entries, each 16 bytes
pub struct CommandTable {
    pub cmd_fis: FisRegH2D,
    _padding: [u8; 44],
    _ata_cmd_area: [u8; 16],
    _reserved: [u8; 0x30],
}

bitfield! {
    #[repr(C)]
    pub struct PrdtEntryFlags(u32);
    impl Debug;
    // this is byte_len - 1, the controller uses 0..=byte_count
    // has to be an odd number
    pub byte_count, set_byte_count: 21, 0;
    pub interrupt, set_interrupt: 31, 31;
}

#[repr(C, packed)]
pub struct PrdtEntry {
    // has to be 2 bytes aligned
    pub data_base_low: u32,
    pub data_base_high: u32,
    _reserved: u32,
    pub flags: PrdtEntryFlags,
}
