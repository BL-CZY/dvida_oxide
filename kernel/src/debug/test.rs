use alloc::vec;

use crate::{drivers::ata::pata::PRIMARY_PATA, println};

#[allow(unconditional_recursion)]
#[allow(unused)]
fn stack_overflow() {
    stack_overflow();
    stack_overflow();
}

#[allow(unused)]
fn page_fault() {
    unsafe {
        *(0xdeadbeef as *mut u8) = 42;
    }
}

#[allow(unused)]
fn binary_test_test() {
    assert!(crate::utils::binary_test(0b001000u64, 3));
    assert!(!crate::utils::binary_test(0b010000u64, 3));
}

#[allow(unused)]
fn pata_pio() {
    let mut input = vec![];
    for _ in 0..256 {
        input.push(10);
        input.push(20);
    }

    if let Err(e) = PRIMARY_PATA.lock().pio_write_sectors(0, 1, &mut input) {
        panic!("failed test pata_pio write {:?}", e);
    }

    let vec = match PRIMARY_PATA.lock().pio_read_sectors(0, 1) {
        Ok(res) => res,
        Err(e) => panic!("failed test pata_pio read {:?}", e),
    };

    assert_eq!(input, vec);

    for i in 0..512 {
        input[i] = 0;
    }

    if let Err(e) = PRIMARY_PATA.lock().pio_write_sectors(0, 1, &mut input) {
        panic!("failed test pata_pio write {:?}", e);
    }

    let mut input = vec![];
    for _ in 0..512 {
        input.push(10);
        input.push(20);
    }

    if let Err(e) = PRIMARY_PATA.lock().pio_write_sectors(0, 2, &mut input) {
        panic!("failed test pata_pio write {:?}", e);
    }

    let vec = match PRIMARY_PATA.lock().pio_read_sectors(0, 2) {
        Ok(res) => res,
        Err(e) => panic!("failed test pata_pio read {:?}", e),
    };

    assert_eq!(input, vec);

    for i in 0..1024 {
        input[i] = 0;
    }

    if let Err(e) = PRIMARY_PATA.lock().pio_write_sectors(0, 1, &mut input) {
        panic!("failed test pata_pio write {:?}", e);
    }
}
