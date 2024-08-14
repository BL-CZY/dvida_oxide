#[cfg(test)]
use crate::drivers::ata::pata::PRIMARY_PATA;
#[cfg(test)]
use alloc::vec;

#[cfg(test)]
#[macro_export]
macro_rules! ignore {
    ($name: expr) => {
        $crate::println!("ignored test: {}", $name);
        return;
    };
}

#[cfg(test)]
#[macro_export]
macro_rules! test_name {
    ($name: expr) => {
        $crate::println!("running test: {}", $name);
    };
}

#[test_case]
#[allow(unreachable_code)]
fn page_fault() {
    ignore!("page fault");
    test_name!("page fault");

    unsafe {
        *(0xdeadbeef as *mut u8) = 42;
    }
}

#[test_case]
#[allow(unreachable_code)]
fn pata_pio() {
    test_name!("PATA PIO r/w");

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
