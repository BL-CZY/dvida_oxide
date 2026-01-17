pub mod apic;
pub mod facp;
pub mod mcfg;

use alloc::{vec, vec::Vec};
use bytemuck::{Pod, Zeroable};
use limine::request::RsdpRequest;
use terminal::log;
use x86_64::VirtAddr;

use crate::arch::x86_64::memory::get_hhdm_offset;

#[derive(Clone, Copy, Pod, Zeroable, Default, Debug)]
#[repr(C, packed)]
pub struct Rsdp {
    signature: [u8; 8],
    checksum: u8,
    oemid: [u8; 6],
    revision: u8,
    rsdt_addr: u32,

    // ACPI 2.0
    length: u32,
    xsdt_addr: u64,
    extended_checksum: u8,
    reserved: [u8; 3],
}

#[derive(Clone, Copy, Pod, Zeroable, Default, Debug)]
#[repr(C, packed)]
pub struct AcpiSdtHeader {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oemid: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
}

pub static RSDP_REQUEST: RsdpRequest = RsdpRequest::new();

pub static RSDP_1_0_LENGTH: usize = 8 + 1 + 6 + 1 + 4;
pub static RSDP_2_0_LENGTH: usize = 4 + 8 + 1 + 3;

const ACPI_2_0: u8 = 2;

fn check_rsdp(rsdp: &Rsdp) {
    let rsdp_buf = bytemuck::bytes_of(rsdp);

    let mut sum = 0;

    for i in 0..RSDP_1_0_LENGTH {
        sum += rsdp_buf[i] as u32;
    }

    if sum & 0xff != 0 {
        panic!("ACPI checksum failed");
    }

    sum = 0;

    for i in RSDP_1_0_LENGTH..RSDP_2_0_LENGTH {
        sum += rsdp_buf[i] as u32;
    }

    assert_eq!(sum & 0xff, 0);
}

fn check_acpi_sdt_header(header: *const AcpiSdtHeader, length: usize) {
    let buf = unsafe { core::slice::from_raw_parts(header as *mut u8, length) };

    let mut sum = 0;

    for i in buf.iter() {
        sum += *i as u32;
    }

    assert_eq!(sum & 0xff, 0);
}

pub fn parse_rsdp() -> Vec<VirtAddr> {
    let response = RSDP_REQUEST.get_response().expect("no rsdp table detected");

    let rsdp = &unsafe { *(response.address() as *const Rsdp) };

    assert_eq!(&rsdp.signature, b"RSD PTR ");

    log!("{:?}", rsdp);

    if rsdp.revision != ACPI_2_0 {
        panic!("Non supported ACPI");
    }

    check_rsdp(&rsdp);

    let xsdt_pointer = (rsdp.xsdt_addr + get_hhdm_offset().as_u64()) as *const AcpiSdtHeader;
    let xsdt_header = &unsafe { *xsdt_pointer };

    check_acpi_sdt_header(xsdt_pointer, xsdt_header.length as usize);

    let num_tables = (xsdt_header.length as usize - size_of::<AcpiSdtHeader>()) / 8;

    let mut xsdt_pointer = VirtAddr::from_ptr(xsdt_pointer);
    xsdt_pointer += size_of::<AcpiSdtHeader>() as u64;

    let mut table_pointers: Vec<VirtAddr> = vec![];

    for i in 0..num_tables {
        let pointer: u32 = unsafe { *((xsdt_pointer + (i as u64 * 8)).as_ptr()) };
        table_pointers.push(VirtAddr::new(pointer as u64) + get_hhdm_offset().as_u64());
    }

    table_pointers
}

pub fn find_table(pointers: &[VirtAddr], signature: [u8; 4]) -> Option<VirtAddr> {
    for addr in pointers.iter() {
        let header: *const AcpiSdtHeader = addr.as_ptr();
        let header = &unsafe { *header };

        if header.signature == signature {
            check_acpi_sdt_header(addr.as_ptr(), header.length as usize);
            return Some(addr.clone());
        }
    }

    None
}

pub fn find_madt(pointers: &[VirtAddr]) -> Option<VirtAddr> {
    find_table(pointers, [b'A', b'P', b'I', b'C'])
}

pub fn find_mcfg(pointers: &[VirtAddr]) -> Option<VirtAddr> {
    find_table(pointers, [b'M', b'C', b'F', b'G'])
}

pub fn find_fadt(pointers: &[VirtAddr]) -> Option<VirtAddr> {
    find_table(pointers, [b'F', b'A', b'C', b'P'])
}
