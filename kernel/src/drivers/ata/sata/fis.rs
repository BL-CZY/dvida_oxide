/// H2D is the controller, and the messages are called FIS
use bitfield::bitfield;
use bytemuck::{Pod, Zeroable};
use smart_default::SmartDefault;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FisType {
    /// Register FIS - host to device
    RegH2D = 0x27,
    /// Register FIS - device to host
    RegD2H = 0x34,
    /// DMA activate FIS - device to host
    DmaAct = 0x39,
    /// DMA setup FIS - bidirectional
    DmaSetup = 0x41,
    /// Data FIS - bidirectional
    Data = 0x46,
    /// BIST activate FIS - bidirectional
    Bist = 0x58,
    /// PIO setup FIS - device to host
    PioSetup = 0x5F,
    /// Set device bits FIS - device to host
    DevBits = 0xA1,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtaCommand {
    /// Read sectors using DMA (48-bit LBA)
    ReadDmaExt = 0x25,
    /// Write sectors using DMA (48-bit LBA)
    WriteDmaExt = 0x35,
    /// Read sectors using DMA (28-bit LBA)
    ReadDma = 0xC8,
    /// Write sectors using DMA (28-bit LBA)
    WriteDma = 0xCA,
    /// Retrieve 512 bytes of device identification data
    Identify = 0xEC,
    /// Flush the drive's internal write cache to physical media
    FlushCache = 0xE7,
    /// Flush the drive's internal write cache (48-bit LBA version)
    FlushCacheExt = 0xEA,
}

bitfield! {
    #[repr(C, packed)]
    pub struct FisRegH2DFlags(u8);

    impl Debug;
    /// Bits 0-3: Port Multiplier
    pub port_multiplier, set_port_multiplier: 3, 0;
    /// Bits 4-6: Reserved (Should be 0)
    pub reserved, _: 6, 4;
    /// Bit 7: 1 for Command, 0 for Control
    pub is_command, set_is_command: 7;
}

#[derive(Pod, Zeroable, Clone, Copy, SmartDefault)]
#[repr(C, packed)]
pub struct FisRegH2D {
    /// Always 0x27 for this type
    #[default = 0x27]
    pub fis_type: u8,
    /// flags
    pub flags: u8,
    /// The ATA Command
    pub command: u8,
    /// Feature register low byte
    pub feature_low: u8,

    /// LBA bits 0-7
    pub lba0: u8,
    /// LBA bits 8-15
    pub lba1: u8,
    /// LBA bits 16-23
    pub lba2: u8,
    /// Device register (bit 6 = 1 for LBA mode)
    pub device: u8,

    /// LBA bits 24-31
    pub lba3: u8,
    /// LBA bits 32-39
    pub lba4: u8,
    /// LBA bits 40-47
    pub lba5: u8,
    /// Feature register high byte
    pub feature_high: u8,

    /// Sector count low byte (0-7)
    pub count_low: u8,
    /// Sector count high byte (8-15)
    pub count_high: u8,
    /// Isochronous command completion
    pub icc: u8,
    /// Control register
    pub control: u8,

    /// Reserved field to maintain 20-byte FIS size
    pub reserved: [u8; 4],
}

bitfield! {
    #[repr(C, packed)]
    pub struct FisRegD2HFlags(u8);
    impl Debug;
    /// Bits 0-3: Port Multiplier
    pub port_multiplier, _: 3, 0;
    // bits 4-5: Reserved
    /// Bit 6: Interrupt Bit (Set when a command completes)
    pub interrupt, _: 6;
    // bit 7: Reserved
}

#[repr(C, packed)]
pub struct FisRegD2H {
    /// FIS_TYPE_REG_D2H (0x34)
    pub fis_type: u8,

    /// Flags including Port Multiplier and Interrupt bit
    pub flags: FisRegD2HFlags,

    /// Status register (e.g., bit 0 is error, bit 7 is busy)
    pub status: u8,
    /// Error register (contains specific error code if status bit 0 is set)
    pub error: u8,

    /// LBA bits 0-7
    pub lba0: u8,
    /// LBA bits 8-15
    pub lba1: u8,
    /// LBA bits 16-23
    pub lba2: u8,
    /// Device register
    pub device: u8,

    /// LBA bits 24-31
    pub lba3: u8,
    /// LBA bits 32-39
    pub lba4: u8,
    /// LBA bits 40-47
    pub lba5: u8,
    /// Reserved
    pub rsv2: u8,

    /// Sector count low byte (0-7)
    pub countl: u8,
    /// Sector count high byte (8-15)
    pub counth: u8,
    /// Reserved
    pub rsv3: [u8; 2],

    /// Reserved
    pub rsv4: [u8; 4],
}

impl FisRegD2H {
    pub fn sector_count(&self) -> u16 {
        ((self.counth as u16) << 8) | (self.countl as u16)
    }
}

bitfield! {
    #[repr(C, packed)]
    pub struct FisDataFlags(u8);
    impl Debug;
    /// Bits 0-3: Port Multiplier
    pub port_multiplier, _: 3, 0;
}

#[repr(C, packed)]
pub struct FisData {
    /// FIS_TYPE_DATA (0x46)
    pub fis_type: u8,
    /// Port multiplier and reserved bits
    pub flags: FisDataFlags,
    /// Reserved
    pub rsv1: [u8; 2],
    /// Payload (Variable length).
    /// In Rust, we usually access this via pointer math rather than a fixed array.
    pub data: [u32; 1],
}

bitfield! {
    #[repr(C, packed)]
    pub struct FisPioSetupFlags(u8);
    impl Debug;
    pub port_multiplier, _: 3, 0;
    // bit 4 reserved
    /// Direction: 1 = Device to Host, 0 = Host to Device
    pub direction, _: 5;
    /// Interrupt bit
    pub interrupt, _: 6;
    // bit 7 reserved
}

#[repr(C, packed)]
pub struct FisPioSetup {
    /// FIS_TYPE_PIO_SETUP (0x5F)
    pub fis_type: u8,
    pub flags: FisPioSetupFlags,
    pub status: u8,
    pub error: u8,

    pub lba0: u8,
    pub lba1: u8,
    pub lba2: u8,
    pub device: u8,

    pub lba3: u8,
    pub lba4: u8,
    pub lba5: u8,
    pub rsv2: u8,

    pub countl: u8,
    pub counth: u8,
    pub rsv3: u8,
    /// New value of status register
    pub e_status: u8,

    /// Transfer count
    pub tc: u16,
    pub rsv4: [u8; 2],
}

bitfield! {
    #[repr(C, packed)]
    pub struct FisDmaSetupFlags(u8);
    impl Debug;
    pub port_multiplier, _: 3, 0;
    // bit 4 reserved
    /// Direction: 1 = Device to Host, 0 = Host to Device
    pub direction, _: 5;
    /// Interrupt bit
    pub interrupt, _: 6;
    /// Auto-activate: Specifies if DMA Activate FIS is needed
    pub auto_activate, _: 7;
}

#[repr(C, packed)]
pub struct FisDmaSetup {
    /// FIS_TYPE_DMA_SETUP (0x41)
    pub fis_type: u8,
    pub flags: FisDmaSetupFlags,
    pub rsved: [u8; 2],

    /// DMA Buffer Identifier
    pub dma_buffer_id: u64,
    pub rsvd: u32,

    /// Byte offset into buffer
    pub dma_buf_offset: u32,
    /// Number of bytes to transfer
    pub transfer_count: u32,
    pub resvd: u32,
}

bitfield! {
    #[repr(C, packed)]
    pub struct FisSetDevBitsFlags(u8);
    impl Debug;
    /// Bits 0-3: Port Multiplier
    pub port_multiplier, _: 3, 0;
    // bit 4-5 reserved
    /// Bit 6: Interrupt bit (If set, the HBA triggers an interrupt to the CPU)
    pub interrupt, _: 6;
    /// Bit 7: Notification bit
    pub notification, _: 7;
}

#[repr(C, packed)]
pub struct FisSetDevBits {
    /// FIS_TYPE_DEV_BITS (0xA1)
    pub fis_type: u8,
    /// Port multiplier and Interrupt/Notification flags
    pub flags: FisSetDevBitsFlags,
    /// The lower 8 bits of the Status register (e.g., Error bit, Busy bit)
    pub status: u8,
    /// The Error register
    pub error: u8,
    /// 32-bit SActive mask (Used for NCQ command completion)
    pub s_active: u32,
}

#[repr(C, align(256))]
pub struct ReceivedFisArea {
    /// Offset 0x00: DMA Setup FIS (28 bytes)
    pub dsfis: FisDmaSetup,
    /// Padding to reach 0x20
    pub reserved0: [u8; 4],

    /// Offset 0x20: PIO Setup FIS (20 bytes)
    pub psfis: FisPioSetup,
    /// Padding to reach 0x40
    pub reserved1: [u8; 12],

    /// Offset 0x40: Register D2H FIS (20 bytes)
    pub rfis: FisRegD2H,
    /// Padding to reach 0x58
    pub reserved2: [u8; 4],

    /// Offset 0x58: Set Device Bits FIS (8 bytes)
    pub sdbfis: FisSetDevBits,

    /// Offset 0x60: Unknown FIS (64 bytes)
    pub ufis: [u8; 64],

    /// Offset 0xA0: Reserved space to round out the 256 bytes
    pub reserved3: [u8; 96],
}
