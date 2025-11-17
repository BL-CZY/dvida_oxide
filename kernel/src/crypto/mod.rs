pub mod crc32;
pub mod guid;
pub mod random;
pub mod uuid;

#[cfg(test)]
mod tests {
    use crate::end_test;
    use crate::ignore;
    use crate::test_name;

    #[test_case]
    #[allow(unreachable_code)]
    fn binary_test_test() {
        ignore!();
        test_name!("binary test function");

        assert!(crate::crypto::binary_test(0b001000u64, 3));
        assert!(!crate::crypto::binary_test(0b010000u64, 3));

        end_test!();
    }
}

pub fn binary_test(target: u64, bit: u64) -> bool {
    (target & (0x1 << bit)) == (0x1 << bit)
}

pub fn lba_to_chs(sectors_per_track: u16, lba: u64) -> (u64, u64, u64) {
    let sectors_per_track: u64 = sectors_per_track as u64;
    let cylinder = lba / (sectors_per_track * 2);
    let head = (lba % (sectors_per_track * 2)) / sectors_per_track;
    let sector = (lba % sectors_per_track) + 1;
    (cylinder, head, sector)
}
