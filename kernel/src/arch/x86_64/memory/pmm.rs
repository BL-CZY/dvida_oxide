use bytemuck::{Pod, Zeroable};

#[derive(PartialEq, Eq, PartialOrd, Ord, Pod, Zeroable, Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct PhysicalPageData {
    pub next: u64,
}
