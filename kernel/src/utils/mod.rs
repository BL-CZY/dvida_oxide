pub fn binary_test(target: u64, bit: u64) -> bool {
    (target & (0x1 << bit)) == (0x1 << bit)
}
