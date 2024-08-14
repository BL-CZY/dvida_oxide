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
