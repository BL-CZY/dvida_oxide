pub struct BitMap {
    pub start: *mut u8,
    /// length in bytes
    pub length: u64,
    /// length in pages
    pub page_length: u64,
}
