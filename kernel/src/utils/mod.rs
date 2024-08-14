pub mod crc32;
pub mod guid;

#[cfg(test)]
use crate::test_name;

#[test_case]
#[allow(unreachable_code)]
fn binary_test_test() {
    test_name!("binary test function");

    assert!(crate::utils::binary_test(0b001000u64, 3));
    assert!(!crate::utils::binary_test(0b010000u64, 3));
}

pub fn binary_test(target: u64, bit: u64) -> bool {
    (target & (0x1 << bit)) == (0x1 << bit)
}
