pub mod pata;

// ATA Ports
pub mod offsets {
    pub const DATA: u16 = 0;
    pub const ERROR: u16 = 1;
    pub const FEATURE: u16 = 1;
    pub const SECTOR_COUNT: u16 = 2;
    pub const LBA_LOW: u16 = 3;
    pub const LBA_MID: u16 = 4;
    pub const LBA_HIGH: u16 = 5;
    pub const DRIVE: u16 = 6;
    pub const COMMAND: u16 = 7;
    pub const STATUS: u16 = 7;
}

// ATA Commands
pub mod cmd {
    pub const READ_SECTORS: u8 = 0x20;
    pub const READ_SECTORS_EXT: u8 = 0x24;
    pub const WRITE_SECTORS: u8 = 0x30;
    pub const WRITE_SECTORS_EXT: u8 = 0x34;
    pub const START_IDENTIFY: u8 = 0xA0;
    pub const IDENTITY: u8 = 0xEC;
    pub const LBA28: u8 = 0xE0;
    pub const LBA48: u8 = 0x40;
    pub const FLUSH_CACHE: u8 = 0xE7;
}
