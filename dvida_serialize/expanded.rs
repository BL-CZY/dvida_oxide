#![feature(prelude_import)]
#![no_std]
#[prelude_import]
use core::prelude::rust_2024::*;
#[macro_use]
extern crate core;
extern crate alloc;
mod numbers {
    use crate::{DvDeErr, DvDeserialize, DvSerErr, DvSerialize, Endianness};
    impl DvSerialize for u8 {
        fn serialize(&self, endianness: Endianness, target: &mut [u8]) -> Result<usize, DvSerErr> {
            const SIZE: usize = core::mem::size_of::<u8>();
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
    impl DvDeserialize for u8 {
        fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
        where
            Self: Sized,
        {
            const SIZE: usize = core::mem::size_of::<u8>();
            if input.len() != SIZE {
                return Err(DvDeErr::WrongBufferSize);
            }
            let mut bytes = [0u8; SIZE];
            bytes.copy_from_slice(&input[..SIZE]);
            let number = match endianness {
                Endianness::NA | Endianness::Little => <u8>::from_le_bytes(bytes),
                Endianness::Big => <u8>::from_be_bytes(bytes),
            };
            Ok((number, SIZE))
        }
    }
    impl DvSerialize for u16 {
        fn serialize(&self, endianness: Endianness, target: &mut [u8]) -> Result<usize, DvSerErr> {
            const SIZE: usize = core::mem::size_of::<u16>();
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
    impl DvDeserialize for u16 {
        fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
        where
            Self: Sized,
        {
            const SIZE: usize = core::mem::size_of::<u16>();
            if input.len() != SIZE {
                return Err(DvDeErr::WrongBufferSize);
            }
            let mut bytes = [0u8; SIZE];
            bytes.copy_from_slice(&input[..SIZE]);
            let number = match endianness {
                Endianness::NA | Endianness::Little => <u16>::from_le_bytes(bytes),
                Endianness::Big => <u16>::from_be_bytes(bytes),
            };
            Ok((number, SIZE))
        }
    }
    impl DvSerialize for u32 {
        fn serialize(&self, endianness: Endianness, target: &mut [u8]) -> Result<usize, DvSerErr> {
            const SIZE: usize = core::mem::size_of::<u32>();
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
    impl DvDeserialize for u32 {
        fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
        where
            Self: Sized,
        {
            const SIZE: usize = core::mem::size_of::<u32>();
            if input.len() != SIZE {
                return Err(DvDeErr::WrongBufferSize);
            }
            let mut bytes = [0u8; SIZE];
            bytes.copy_from_slice(&input[..SIZE]);
            let number = match endianness {
                Endianness::NA | Endianness::Little => <u32>::from_le_bytes(bytes),
                Endianness::Big => <u32>::from_be_bytes(bytes),
            };
            Ok((number, SIZE))
        }
    }
    impl DvSerialize for u64 {
        fn serialize(&self, endianness: Endianness, target: &mut [u8]) -> Result<usize, DvSerErr> {
            const SIZE: usize = core::mem::size_of::<u64>();
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
    impl DvDeserialize for u64 {
        fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
        where
            Self: Sized,
        {
            const SIZE: usize = core::mem::size_of::<u64>();
            if input.len() != SIZE {
                return Err(DvDeErr::WrongBufferSize);
            }
            let mut bytes = [0u8; SIZE];
            bytes.copy_from_slice(&input[..SIZE]);
            let number = match endianness {
                Endianness::NA | Endianness::Little => <u64>::from_le_bytes(bytes),
                Endianness::Big => <u64>::from_be_bytes(bytes),
            };
            Ok((number, SIZE))
        }
    }
    impl DvSerialize for u128 {
        fn serialize(&self, endianness: Endianness, target: &mut [u8]) -> Result<usize, DvSerErr> {
            const SIZE: usize = core::mem::size_of::<u128>();
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
    impl DvDeserialize for u128 {
        fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
        where
            Self: Sized,
        {
            const SIZE: usize = core::mem::size_of::<u128>();
            if input.len() != SIZE {
                return Err(DvDeErr::WrongBufferSize);
            }
            let mut bytes = [0u8; SIZE];
            bytes.copy_from_slice(&input[..SIZE]);
            let number = match endianness {
                Endianness::NA | Endianness::Little => <u128>::from_le_bytes(bytes),
                Endianness::Big => <u128>::from_be_bytes(bytes),
            };
            Ok((number, SIZE))
        }
    }
    impl DvSerialize for i8 {
        fn serialize(&self, endianness: Endianness, target: &mut [u8]) -> Result<usize, DvSerErr> {
            const SIZE: usize = core::mem::size_of::<i8>();
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
    impl DvDeserialize for i8 {
        fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
        where
            Self: Sized,
        {
            const SIZE: usize = core::mem::size_of::<i8>();
            if input.len() != SIZE {
                return Err(DvDeErr::WrongBufferSize);
            }
            let mut bytes = [0u8; SIZE];
            bytes.copy_from_slice(&input[..SIZE]);
            let number = match endianness {
                Endianness::NA | Endianness::Little => <i8>::from_le_bytes(bytes),
                Endianness::Big => <i8>::from_be_bytes(bytes),
            };
            Ok((number, SIZE))
        }
    }
    impl DvSerialize for i16 {
        fn serialize(&self, endianness: Endianness, target: &mut [u8]) -> Result<usize, DvSerErr> {
            const SIZE: usize = core::mem::size_of::<i16>();
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
    impl DvDeserialize for i16 {
        fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
        where
            Self: Sized,
        {
            const SIZE: usize = core::mem::size_of::<i16>();
            if input.len() != SIZE {
                return Err(DvDeErr::WrongBufferSize);
            }
            let mut bytes = [0u8; SIZE];
            bytes.copy_from_slice(&input[..SIZE]);
            let number = match endianness {
                Endianness::NA | Endianness::Little => <i16>::from_le_bytes(bytes),
                Endianness::Big => <i16>::from_be_bytes(bytes),
            };
            Ok((number, SIZE))
        }
    }
    impl DvSerialize for i32 {
        fn serialize(&self, endianness: Endianness, target: &mut [u8]) -> Result<usize, DvSerErr> {
            const SIZE: usize = core::mem::size_of::<i32>();
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
    impl DvDeserialize for i32 {
        fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
        where
            Self: Sized,
        {
            const SIZE: usize = core::mem::size_of::<i32>();
            if input.len() != SIZE {
                return Err(DvDeErr::WrongBufferSize);
            }
            let mut bytes = [0u8; SIZE];
            bytes.copy_from_slice(&input[..SIZE]);
            let number = match endianness {
                Endianness::NA | Endianness::Little => <i32>::from_le_bytes(bytes),
                Endianness::Big => <i32>::from_be_bytes(bytes),
            };
            Ok((number, SIZE))
        }
    }
    impl DvSerialize for i64 {
        fn serialize(&self, endianness: Endianness, target: &mut [u8]) -> Result<usize, DvSerErr> {
            const SIZE: usize = core::mem::size_of::<i64>();
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
    impl DvDeserialize for i64 {
        fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
        where
            Self: Sized,
        {
            const SIZE: usize = core::mem::size_of::<i64>();
            if input.len() != SIZE {
                return Err(DvDeErr::WrongBufferSize);
            }
            let mut bytes = [0u8; SIZE];
            bytes.copy_from_slice(&input[..SIZE]);
            let number = match endianness {
                Endianness::NA | Endianness::Little => <i64>::from_le_bytes(bytes),
                Endianness::Big => <i64>::from_be_bytes(bytes),
            };
            Ok((number, SIZE))
        }
    }
    impl DvSerialize for i128 {
        fn serialize(&self, endianness: Endianness, target: &mut [u8]) -> Result<usize, DvSerErr> {
            const SIZE: usize = core::mem::size_of::<i128>();
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
    impl DvDeserialize for i128 {
        fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
        where
            Self: Sized,
        {
            const SIZE: usize = core::mem::size_of::<i128>();
            if input.len() != SIZE {
                return Err(DvDeErr::WrongBufferSize);
            }
            let mut bytes = [0u8; SIZE];
            bytes.copy_from_slice(&input[..SIZE]);
            let number = match endianness {
                Endianness::NA | Endianness::Little => <i128>::from_le_bytes(bytes),
                Endianness::Big => <i128>::from_be_bytes(bytes),
            };
            Ok((number, SIZE))
        }
    }
    impl DvSerialize for f32 {
        fn serialize(&self, endianness: Endianness, target: &mut [u8]) -> Result<usize, DvSerErr> {
            const SIZE: usize = core::mem::size_of::<f32>();
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
    impl DvDeserialize for f32 {
        fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
        where
            Self: Sized,
        {
            const SIZE: usize = core::mem::size_of::<f32>();
            if input.len() != SIZE {
                return Err(DvDeErr::WrongBufferSize);
            }
            let mut bytes = [0u8; SIZE];
            bytes.copy_from_slice(&input[..SIZE]);
            let number = match endianness {
                Endianness::NA | Endianness::Little => <f32>::from_le_bytes(bytes),
                Endianness::Big => <f32>::from_be_bytes(bytes),
            };
            Ok((number, SIZE))
        }
    }
    impl DvSerialize for f64 {
        fn serialize(&self, endianness: Endianness, target: &mut [u8]) -> Result<usize, DvSerErr> {
            const SIZE: usize = core::mem::size_of::<f64>();
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
    impl DvDeserialize for f64 {
        fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
        where
            Self: Sized,
        {
            const SIZE: usize = core::mem::size_of::<f64>();
            if input.len() != SIZE {
                return Err(DvDeErr::WrongBufferSize);
            }
            let mut bytes = [0u8; SIZE];
            bytes.copy_from_slice(&input[..SIZE]);
            let number = match endianness {
                Endianness::NA | Endianness::Little => <f64>::from_le_bytes(bytes),
                Endianness::Big => <f64>::from_be_bytes(bytes),
            };
            Ok((number, SIZE))
        }
    }
}
pub use dvida_serialize_macros::DvDeSer;
pub enum Endianness {
    Little,
    Big,
    NA,
}
#[automatically_derived]
impl ::core::clone::Clone for Endianness {
    #[inline]
    fn clone(&self) -> Endianness {
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for Endianness {}
#[automatically_derived]
impl ::core::fmt::Debug for Endianness {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::write_str(
            f,
            match self {
                Endianness::Little => "Little",
                Endianness::Big => "Big",
                Endianness::NA => "NA",
            },
        )
    }
}
pub enum DvSerErr {
    BufferTooSmall,
}
#[automatically_derived]
impl ::core::fmt::Debug for DvSerErr {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::write_str(f, "BufferTooSmall")
    }
}
#[automatically_derived]
impl ::core::clone::Clone for DvSerErr {
    #[inline]
    fn clone(&self) -> DvSerErr {
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for DvSerErr {}
pub enum DvDeErr {
    WrongBufferSize,
}
#[automatically_derived]
impl ::core::fmt::Debug for DvDeErr {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::write_str(f, "WrongBufferSize")
    }
}
#[automatically_derived]
impl ::core::clone::Clone for DvDeErr {
    #[inline]
    fn clone(&self) -> DvDeErr {
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for DvDeErr {}
pub trait DvSerialize {
    /// the serialize function takes in self, endianness, writes data to a slice of data
    /// return the amount of bytes written
    /// it will error if the buffer is too small
    fn serialize(&self, endianness: Endianness, target: &mut [u8]) -> Result<usize, DvSerErr>;
}
pub trait DvDeserialize {
    /// the deserialize function takes in endianness, a slice of data, and returns the parsed self
    /// and number of bytes read
    /// it will error if the conversion goes wrong
    fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
    where
        Self: Sized;
}
pub struct TestStruct {
    size: u32,
    value: u32,
}
impl DvSerialize for TestStruct {
    fn serialize(&self, endianness: Endianness, target: &mut [u8]) -> Result<usize, DvSerErr> {
        let mut acc: usize = 0;
        acc += self.size.serialize(endianness, &mut target[acc..])?;
        acc += self.value.serialize(endianness, &mut target[acc..])?;
        Ok(acc)
    }
}
impl DvDeserialize for TestStruct {
    fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
    where
        Self: Sized,
    {
        let mut acc: usize = 0;
        let (size, written) = u32::deserialize(endianness, &input[acc..])?;
        acc += written;
        let (value, written) = u32::deserialize(endianness, &input[acc..])?;
        acc += written;
        Ok((Self { size, value }, acc))
    }
}
pub struct EgStruct {
    size: u32,
    value: u32,
}
impl DvSerialize for EgStruct {
    fn serialize(&self, endianness: Endianness, target: &mut [u8]) -> Result<usize, DvSerErr> {
        let mut acc: usize = 0;
        acc += self.size.serialize(endianness, &mut target[acc..])?;
        acc += self.value.serialize(endianness, &mut target[acc..])?;
        Ok(acc)
    }
}
impl DvDeserialize for EgStruct {
    fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
    where
        Self: Sized,
    {
        let mut acc: usize = 0;
        let (size, written) = u32::deserialize(endianness, &input[acc..])?;
        acc += written;
        let (value, written) = u32::deserialize(endianness, &input[acc..])?;
        acc += written;
        Ok((Self { size, value }, acc))
    }
}
mod test {
    use super::*;
    use alloc::vec;
    extern crate test;
    #[rustc_test_marker = "test::test_simple_struct"]
    #[doc(hidden)]
    pub const test_simple_struct: test::TestDescAndFn = test::TestDescAndFn {
        desc: test::TestDesc {
            name: test::StaticTestName("test::test_simple_struct"),
            ignore: false,
            ignore_message: ::core::option::Option::None,
            source_file: "src/lib.rs",
            start_line: 86usize,
            start_col: 8usize,
            end_line: 86usize,
            end_col: 26usize,
            compile_fail: false,
            no_run: false,
            should_panic: test::ShouldPanic::No,
            test_type: test::TestType::UnitTest,
        },
        testfn: test::StaticTestFn(
            #[coverage(off)]
            || test::assert_test_result(test_simple_struct()),
        ),
    };
    fn test_simple_struct() {
        struct Simple {
            size: u32,
            value: u16,
        }
        impl DvSerialize for Simple {
            fn serialize(
                &self,
                endianness: Endianness,
                target: &mut [u8],
            ) -> Result<usize, DvSerErr> {
                let mut acc: usize = 0;
                acc += self.size.serialize(endianness, &mut target[acc..])?;
                acc += self.value.serialize(endianness, &mut target[acc..])?;
                Ok(acc)
            }
        }
        impl DvDeserialize for Simple {
            fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
            where
                Self: Sized,
            {
                let mut acc: usize = 0;
                let (size, written) = u32::deserialize(endianness, &input[acc..])?;
                acc += written;
                let (value, written) = u16::deserialize(endianness, &input[acc..])?;
                acc += written;
                Ok((Self { size, value }, acc))
            }
        }
        let simple = Simple {
            size: 42,
            value: 100,
        };
        let mut buffer = ::alloc::vec::from_elem(0u8, 10);
        let written = simple.serialize(Endianness::Little, &mut buffer).unwrap();
        match (&written, &6) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = ::core::panicking::AssertKind::Eq;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
        let (deserialized, read) = Simple::deserialize(Endianness::Little, &buffer).unwrap();
        match (&read, &6) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = ::core::panicking::AssertKind::Eq;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
        match (&deserialized.size, &42) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = ::core::panicking::AssertKind::Eq;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
        match (&deserialized.value, &100) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = ::core::panicking::AssertKind::Eq;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
    }
    extern crate test;
    #[rustc_test_marker = "test::test_multiple_fields"]
    #[doc(hidden)]
    pub const test_multiple_fields: test::TestDescAndFn = test::TestDescAndFn {
        desc: test::TestDesc {
            name: test::StaticTestName("test::test_multiple_fields"),
            ignore: false,
            ignore_message: ::core::option::Option::None,
            source_file: "src/lib.rs",
            start_line: 109usize,
            start_col: 8usize,
            end_line: 109usize,
            end_col: 28usize,
            compile_fail: false,
            no_run: false,
            should_panic: test::ShouldPanic::No,
            test_type: test::TestType::UnitTest,
        },
        testfn: test::StaticTestFn(
            #[coverage(off)]
            || test::assert_test_result(test_multiple_fields()),
        ),
    };
    fn test_multiple_fields() {
        struct Multi {
            field1: u32,
            field2: u32,
            field3: u16,
        }
        impl DvSerialize for Multi {
            fn serialize(
                &self,
                endianness: Endianness,
                target: &mut [u8],
            ) -> Result<usize, DvSerErr> {
                let mut acc: usize = 0;
                acc += self.field1.serialize(endianness, &mut target[acc..])?;
                acc += self.field2.serialize(endianness, &mut target[acc..])?;
                acc += self.field3.serialize(endianness, &mut target[acc..])?;
                Ok(acc)
            }
        }
        impl DvDeserialize for Multi {
            fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
            where
                Self: Sized,
            {
                let mut acc: usize = 0;
                let (field1, written) = u32::deserialize(endianness, &input[acc..])?;
                acc += written;
                let (field2, written) = u32::deserialize(endianness, &input[acc..])?;
                acc += written;
                let (field3, written) = u16::deserialize(endianness, &input[acc..])?;
                acc += written;
                Ok((
                    Self {
                        field1,
                        field2,
                        field3,
                    },
                    acc,
                ))
            }
        }
        let multi = Multi {
            field1: 1,
            field2: 2,
            field3: 3,
        };
        let mut buffer = ::alloc::vec::from_elem(0u8, 20);
        let written = multi.serialize(Endianness::Big, &mut buffer).unwrap();
        match (&written, &10) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = ::core::panicking::AssertKind::Eq;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
        let (deserialized, read) = Multi::deserialize(Endianness::Big, &buffer).unwrap();
        match (&read, &10) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = ::core::panicking::AssertKind::Eq;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
        match (&deserialized.field1, &1) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = ::core::panicking::AssertKind::Eq;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
        match (&deserialized.field2, &2) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = ::core::panicking::AssertKind::Eq;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
        match (&deserialized.field3, &3) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = ::core::panicking::AssertKind::Eq;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
    }
    extern crate test;
    #[rustc_test_marker = "test::test_generic_struct_no_where_clause"]
    #[doc(hidden)]
    pub const test_generic_struct_no_where_clause: test::TestDescAndFn = test::TestDescAndFn {
        desc: test::TestDesc {
            name: test::StaticTestName("test::test_generic_struct_no_where_clause"),
            ignore: false,
            ignore_message: ::core::option::Option::None,
            source_file: "src/lib.rs",
            start_line: 135usize,
            start_col: 8usize,
            end_line: 135usize,
            end_col: 43usize,
            compile_fail: false,
            no_run: false,
            should_panic: test::ShouldPanic::No,
            test_type: test::TestType::UnitTest,
        },
        testfn: test::StaticTestFn(
            #[coverage(off)]
            || test::assert_test_result(test_generic_struct_no_where_clause()),
        ),
    };
    fn test_generic_struct_no_where_clause() {
        struct GenericPair<T, U> {
            first: T,
            second: U,
        }
        impl<T, U> DvSerialize for GenericPair<T, U> {
            fn serialize(
                &self,
                endianness: Endianness,
                target: &mut [u8],
            ) -> Result<usize, DvSerErr> {
                let mut acc: usize = 0;
                acc += self.first.serialize(endianness, &mut target[acc..])?;
                acc += self.second.serialize(endianness, &mut target[acc..])?;
                Ok(acc)
            }
        }
        impl<T, U> DvDeserialize for GenericPair<T, U> {
            fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
            where
                Self: Sized,
            {
                let mut acc: usize = 0;
                let (first, written) = T::deserialize(endianness, &input[acc..])?;
                acc += written;
                let (second, written) = U::deserialize(endianness, &input[acc..])?;
                acc += written;
                Ok((Self { first, second }, acc))
            }
        }
        let pair: GenericPair<u32, u16> = GenericPair {
            first: 123,
            second: 456,
        };
        let mut buffer = ::alloc::vec::from_elem(0u8, 10);
        let written = pair.serialize(Endianness::Little, &mut buffer).unwrap();
        match (&written, &6) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = ::core::panicking::AssertKind::Eq;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
        let (deserialized, read) =
            GenericPair::<u32, u16>::deserialize(Endianness::Little, &buffer).unwrap();
        match (&read, &6) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = ::core::panicking::AssertKind::Eq;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
        match (&deserialized.first, &123) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = ::core::panicking::AssertKind::Eq;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
        match (&deserialized.second, &456) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = ::core::panicking::AssertKind::Eq;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
    }

    extern crate test;
    #[rustc_test_marker = "test::test_generic_struct_with_where_clause"]
    #[doc(hidden)]
    pub const test_generic_struct_with_where_clause: test::TestDescAndFn = test::TestDescAndFn {
        desc: test::TestDesc {
            name: test::StaticTestName("test::test_generic_struct_with_where_clause"),
            ignore: false,
            ignore_message: ::core::option::Option::None,
            source_file: "src/lib.rs",
            start_line: 159usize,
            start_col: 8usize,
            end_line: 159usize,
            end_col: 45usize,
            compile_fail: false,
            no_run: false,
            should_panic: test::ShouldPanic::No,
            test_type: test::TestType::UnitTest,
        },
        testfn: test::StaticTestFn(
            #[coverage(off)]
            || test::assert_test_result(test_generic_struct_with_where_clause()),
        ),
    };
    fn test_generic_struct_with_where_clause() {
        struct BoundedPair<T, U>
        where
            T: DvSerialize + DvDeserialize,
            U: DvSerialize + DvDeserialize,
        {
            first: T,
            second: U,
        }
        impl<T, U> DvSerialize for BoundedPair<T, U>
        where
            T: DvSerialize + DvDeserialize,
            U: DvSerialize + DvDeserialize,
        {
            fn serialize(
                &self,
                endianness: Endianness,
                target: &mut [u8],
            ) -> Result<usize, DvSerErr> {
                let mut acc: usize = 0;
                acc += self.first.serialize(endianness, &mut target[acc..])?;
                acc += self.second.serialize(endianness, &mut target[acc..])?;
                Ok(acc)
            }
        }
        impl<T, U> DvDeserialize for BoundedPair<T, U>
        where
            T: DvSerialize + DvDeserialize,
            U: DvSerialize + DvDeserialize,
        {
            fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
            where
                Self: Sized,
            {
                let mut acc: usize = 0;
                let (first, written) = T::deserialize(endianness, &input[acc..])?;
                acc += written;
                let (second, written) = U::deserialize(endianness, &input[acc..])?;
                acc += written;
                Ok((Self { first, second }, acc))
            }
        }

        let pair: BoundedPair<u32, u32> = BoundedPair {
            first: 999,
            second: 888,
        };
        let mut buffer = ::alloc::vec::from_elem(0u8, 10);
        let written = pair.serialize(Endianness::Big, &mut buffer).unwrap();
        match (&written, &8) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = ::core::panicking::AssertKind::Eq;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
        let (deserialized, read) =
            BoundedPair::<u32, u32>::deserialize(Endianness::Big, &buffer).unwrap();
        match (&read, &8) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = ::core::panicking::AssertKind::Eq;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
        match (&deserialized.first, &999) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = ::core::panicking::AssertKind::Eq;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
        match (&deserialized.second, &888) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = ::core::panicking::AssertKind::Eq;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
    }
    extern crate test;
    #[rustc_test_marker = "test::test_single_field_struct"]
    #[doc(hidden)]
    pub const test_single_field_struct: test::TestDescAndFn = test::TestDescAndFn {
        desc: test::TestDesc {
            name: test::StaticTestName("test::test_single_field_struct"),
            ignore: false,
            ignore_message: ::core::option::Option::None,
            source_file: "src/lib.rs",
            start_line: 187usize,
            start_col: 8usize,
            end_line: 187usize,
            end_col: 32usize,
            compile_fail: false,
            no_run: false,
            should_panic: test::ShouldPanic::No,
            test_type: test::TestType::UnitTest,
        },
        testfn: test::StaticTestFn(
            #[coverage(off)]
            || test::assert_test_result(test_single_field_struct()),
        ),
    };
    fn test_single_field_struct() {
        struct Single {
            value: u32,
        }
        impl DvSerialize for Single {
            fn serialize(
                &self,
                endianness: Endianness,
                target: &mut [u8],
            ) -> Result<usize, DvSerErr> {
                let mut acc: usize = 0;
                acc += self.value.serialize(endianness, &mut target[acc..])?;
                Ok(acc)
            }
        }
        impl DvDeserialize for Single {
            fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
            where
                Self: Sized,
            {
                let mut acc: usize = 0;
                let (value, written) = u32::deserialize(endianness, &input[acc..])?;
                acc += written;
                Ok((Self { value }, acc))
            }
        }
        let single = Single { value: 0xDEADBEEF };
        let mut buffer = ::alloc::vec::from_elem(0u8, 4);
        let written = single.serialize(Endianness::Little, &mut buffer).unwrap();
        match (&written, &4) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = ::core::panicking::AssertKind::Eq;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
        let (deserialized, read) = Single::deserialize(Endianness::Little, &buffer).unwrap();
        match (&read, &4) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = ::core::panicking::AssertKind::Eq;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
        match (&deserialized.value, &0xDEADBEEF) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = ::core::panicking::AssertKind::Eq;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
    }
    extern crate test;
    #[rustc_test_marker = "test::test_endianness_matters"]
    #[doc(hidden)]
    pub const test_endianness_matters: test::TestDescAndFn = test::TestDescAndFn {
        desc: test::TestDesc {
            name: test::StaticTestName("test::test_endianness_matters"),
            ignore: false,
            ignore_message: ::core::option::Option::None,
            source_file: "src/lib.rs",
            start_line: 205usize,
            start_col: 8usize,
            end_line: 205usize,
            end_col: 31usize,
            compile_fail: false,
            no_run: false,
            should_panic: test::ShouldPanic::No,
            test_type: test::TestType::UnitTest,
        },
        testfn: test::StaticTestFn(
            #[coverage(off)]
            || test::assert_test_result(test_endianness_matters()),
        ),
    };
    fn test_endianness_matters() {
        struct Data {
            value: u32,
        }
        impl DvSerialize for Data {
            fn serialize(
                &self,
                endianness: Endianness,
                target: &mut [u8],
            ) -> Result<usize, DvSerErr> {
                let mut acc: usize = 0;
                acc += self.value.serialize(endianness, &mut target[acc..])?;
                Ok(acc)
            }
        }
        impl DvDeserialize for Data {
            fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
            where
                Self: Sized,
            {
                let mut acc: usize = 0;
                let (value, written) = u32::deserialize(endianness, &input[acc..])?;
                acc += written;
                Ok((Self { value }, acc))
            }
        }
        let data = Data { value: 0x12345678 };
        let mut buffer_le = ::alloc::vec::from_elem(0u8, 4);
        let mut buffer_be = ::alloc::vec::from_elem(0u8, 4);
        data.serialize(Endianness::Little, &mut buffer_le).unwrap();
        data.serialize(Endianness::Big, &mut buffer_be).unwrap();
        match (&buffer_le, &buffer_be) {
            (left_val, right_val) => {
                if *left_val == *right_val {
                    let kind = ::core::panicking::AssertKind::Ne;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
        let (deser_le, _) = Data::deserialize(Endianness::Little, &buffer_le).unwrap();
        let (deser_be, _) = Data::deserialize(Endianness::Big, &buffer_be).unwrap();
        match (&deser_le.value, &0x12345678) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = ::core::panicking::AssertKind::Eq;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
        match (&deser_be.value, &0x12345678) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = ::core::panicking::AssertKind::Eq;
                    ::core::panicking::assert_failed(
                        kind,
                        &*left_val,
                        &*right_val,
                        ::core::option::Option::None,
                    );
                }
            }
        };
    }
}
#[rustc_main]
#[coverage(off)]
#[doc(hidden)]
pub fn main() -> () {
    extern crate test;
    test::test_main_static(&[
        &test_endianness_matters,
        &test_generic_struct_no_where_clause,
        &test_generic_struct_with_where_clause,
        &test_multiple_fields,
        &test_simple_struct,
        &test_single_field_struct,
    ])
}
