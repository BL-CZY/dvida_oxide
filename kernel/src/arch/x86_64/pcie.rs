use x86_64::VirtAddr;

#[macro_export]
macro_rules! pcie_port_readonly {
    ($name:ident, $output_type:ty, | $self:ident | $addr:block) => {
        paste::paste! {
            pub fn [<read_$name>](&$self) -> $output_type {
                let address: *mut $output_type = $addr;
                unsafe { address.read_volatile() }
            }
        }
    };

    ($name:ident, $output_type:ty, | $self:ident | $addr:block, || $head:block, || $tail:block) => {
        paste::paste! {
            pub fn [<read_$name>](&$self) -> $output_type {
                $head;

                let address: *mut $output_type = $addr;
                let res = unsafe { address.read_volatile() };

                $tail;

                res
            }
        }
    };
}

#[macro_export]
macro_rules! pcie_port_writeonly {
    ($name:ident, $input_type:ty, | $self:ident | $addr:block) => {
        paste::paste! {
            pub fn [<write_$name>](&mut $self, input: $input_type) {
                let address: *mut $input_type = $addr;
                unsafe { address.write_volatile(input) }
            }
        }
    };

    ($name:ident, $input_type:ty, | $self:ident | $addr:block, || $head:block, || $tail:block) => {
        paste::paste! {
            pub fn [<write_$name>](&mut $self, input: $input_type) {
                $head;

                let address: *mut $input_type = $addr;
                unsafe { address.write_volatile(input) }

                $tail;
            }
        }
    };
}

#[macro_export]
macro_rules! pcie_port_readwrite {
    ($($args:tt)*) => {
        $crate::pcie_port_readonly!($($args)*);
        $crate::pcie_port_writeonly!($($args)*);
    };
}

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct PciHeaderPartial {
    pub vendor_id: u16,
    pub device_id: u16,
    pub command: u16,
    pub status: u16,
    pub revision_id: u8,
    pub prog_if: u8,
    pub subclass: u8,
    pub class_code: u8,
}

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct PciHeader {
    pub vendor_id: u16,
    pub device_id: u16,
    pub command: u16,
    pub status: u16,
    pub revision_id: u8,
    pub prog_if: u8,
    pub subclass: u8,
    pub class_code: u8,
    pub cache_line_size: u8,
    pub latency_timer: u8,
    pub header_type: u8,
    pub bist: u8,
    // Base Address Registers (BARs)
    pub bars: [u32; 6],
    pub cardbus_cis_ptr: u32,
    pub subsystem_vendor_id: u16,
    pub subsystem_id: u16,
    pub expansion_rom_base_addr: u32,
    pub capabilities_ptr: u8,
    pub reserved: [u8; 7],
    pub interrupt_line: u8,
    pub interrupt_pin: u8,
    pub min_grant: u8,
    pub max_latency: u8,
}

#[repr(u8)]
pub enum PciBaseClass {
    Unclassified = 0x00,
    MassStorage = 0x01,
    Network = 0x02,
    Display = 0x03,
    Multimedia = 0x04,
    Memory = 0x05,
    Bridge = 0x06,
    SimpleComm = 0x07,
    BaseSystemPeripheral = 0x08,
    InputDevice = 0x09,
    DockingStation = 0x0A,
    Processor = 0x0B,
    SerialBus = 0x0C,
    Wireless = 0x0D,
    IntelligentIO = 0x0E,
    SatelliteComm = 0x0F,
    Encryption = 0x10,
    SignalProcessing = 0x11,
    ProcessingAccelerator = 0x12,
    NonEssentialInstrumentation = 0x13,
    CoProcessor = 0x40,
    Unassigned = 0xFF,
}

#[repr(u8)]
pub enum MassStorageControllerSubClass {
    Sata = 0x06,
}

#[repr(u8)]
pub enum SataProgIf {
    Ahci = 0x01,
}

#[derive(Debug, Clone)]
pub struct PciDevice {
    pub address: VirtAddr,
    pub header_partial: PciHeaderPartial,
}
