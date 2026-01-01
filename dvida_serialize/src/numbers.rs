use crate::{DvDeErr, DvDeserialize, DvSerErr, DvSerialize, Endianness};

// Your existing macro for primitives
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
                fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
                where
                    Self: Sized,
                {
                    const SIZE: usize = core::mem::size_of::<$t>();
                    if input.len() < SIZE {
                        return Err(DvDeErr::WrongBufferSize);
                    }
                    let mut bytes = [0u8; SIZE];
                    bytes.copy_from_slice(&input[..SIZE]);
                    let number = match endianness {
                        Endianness::NA | Endianness::Little => <$t>::from_le_bytes(bytes),
                        Endianness::Big => <$t>::from_be_bytes(bytes),
                    };
                    Ok((number, SIZE))
                }
            }
        )*
    };
}

// New macro for arrays of const size
macro_rules! impl_serialize_deserialize_array {
    ($($t:ty),*) => {
        $(
            impl<const N: usize> DvSerialize for [$t; N] {
                fn serialize(&self, endianness: Endianness, target: &mut [u8]) -> Result<usize, DvSerErr> {
                    const ELEM_SIZE: usize = core::mem::size_of::<$t>();
                    let total_size = ELEM_SIZE * N;

                    if target.len() < total_size {
                        return Err(DvSerErr::BufferTooSmall);
                    }

                    for (i, elem) in self.iter().enumerate() {
                        let offset = i * ELEM_SIZE;
                        elem.serialize(endianness, &mut target[offset..offset + ELEM_SIZE])?;
                    }

                    Ok(total_size)
                }
            }

            impl<const N: usize> DvDeserialize for [$t; N] {
                fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
                where
                    Self: Sized,
                {
                    const ELEM_SIZE: usize = core::mem::size_of::<$t>();
                    let total_size = ELEM_SIZE * N;

                    if input.len() < total_size {
                        return Err(DvDeErr::WrongBufferSize);
                    }

                    let mut result = [<$t>::default(); N];

                    for i in 0..N {
                        let offset = i * ELEM_SIZE;
                        let (elem, _) = <$t>::deserialize(endianness, &input[offset..offset + ELEM_SIZE])?;
                        result[i] = elem;
                    }

                    Ok((result, total_size))
                }
            }
        )*
    };
}

// Apply to primitives
impl_serialize_deserialize!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64);

// Apply to arrays of primitives
impl_serialize_deserialize_array!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64);

// #[derive(DvDeSer)]
// struct Test {
//     field1: [u8; 16],
// }
