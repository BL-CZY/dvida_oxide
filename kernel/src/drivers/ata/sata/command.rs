use bitfield::bitfield;

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
