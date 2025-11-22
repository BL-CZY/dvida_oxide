use core::ops::{Add, BitAnd, BitOr, Shl, Sub};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Bit {
    Zero = 0,
    One = 1,
}

pub struct BitIterator<'a, T>
where
    T: Add<Output = T>
        + Sub<Output = T>
        + BitAnd<Output = T>
        + BitOr<Output = T>
        + Shl<usize, Output = T>
        + From<u8>
        + PartialEq
        + Copy,
{
    pub num: &'a mut [T],
    pub idx: usize,
    pub bit_idx: usize,
}

impl<'a, T> BitIterator<'a, T>
where
    T: Add<Output = T>
        + Sub<Output = T>
        + BitAnd<Output = T>
        + BitOr<Output = T>
        + Shl<usize, Output = T>
        + From<u8>
        + PartialEq
        + Copy,
{
    pub fn new(num: &'a mut [T]) -> Self {
        Self {
            num,
            idx: 0,
            bit_idx: 0,
        }
    }
}

impl<'a, T> Iterator for BitIterator<'a, T>
where
    T: Add<Output = T>
        + Sub<Output = T>
        + BitAnd<Output = T>
        + BitOr<Output = T>
        + Shl<usize, Output = T>
        + From<u8>
        + PartialEq
        + Copy,
{
    type Item = Bit;

    fn next(&mut self) -> Option<Self::Item> {
        // Check bounds
        if self.idx >= self.num.len() {
            return None;
        }

        // Create a mask: (1 << bit_idx)
        let mask = T::from(1u8) << self.bit_idx;

        // Check if the bit is set
        let res = self.num[self.idx] & mask;
        let bit = if res == T::from(0u8) {
            Bit::Zero
        } else {
            Bit::One
        };

        // Move to next bit
        self.bit_idx += 1;

        // If we've processed all bits in current element, move to next
        // Assuming T is a fixed-width integer type (adjust bit width as needed)
        let bits_per_element = core::mem::size_of::<T>() * 8;
        if self.bit_idx >= bits_per_element {
            self.bit_idx = 0;
            self.idx += 1;
        }

        Some(bit)
    }
}
