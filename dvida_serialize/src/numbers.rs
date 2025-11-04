use crate::{DvDeErr, DvDeserialize, DvSerErr, DvSerialize, Endianness};

macro_rules! impl_serialize_deserialize {
    ($($t:ty),*) => {
        $(
            impl DvSerialize for $t {
                fn serialize(&self, endianness: Endianness, target: &mut [u8]) -> Result<usize, DvSerErr> {
                    const SIZE: usize = core::mem::size_of::<$t>();
                    if target.len() < SIZE {
                        return Err(DvSerErr::BufferTooSmall);
                    }
                    let bytes = match endianness {
                        Endianness::NA | Endianness::Little => self.to_le_bytes(),
                        Endianness::Big => self.to_be_bytes(),
                    };
                    target[..SIZE].copy_from_slice(&bytes);
                    Ok(SIZE)
                }
            }

            impl DvDeserialize for $t {
                fn deserialize(endianness: Endianness, input: &[u8]) -> Result<Self, DvDeErr>
                where
                    Self: Sized,
                {
                    const SIZE: usize = core::mem::size_of::<$t>();
                    if input.len() != SIZE {
                        return Err(DvDeErr::WrongBufferSize);
                    }
                    let mut bytes = [0u8; SIZE];
                    bytes.copy_from_slice(&input[..SIZE]);
                    let number = match endianness {
                        Endianness::NA | Endianness::Little => <$t>::from_le_bytes(bytes),
                        Endianness::Big => <$t>::from_be_bytes(bytes),
                    };
                    Ok(number)
                }
            }
        )*
    };
}

// Apply to all integer types
impl_serialize_deserialize!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64);
