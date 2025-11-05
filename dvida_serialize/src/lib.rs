#![no_std]

mod numbers;

pub use dvida_serialize_macros::DvDeSer;

#[derive(Clone, Copy, Debug)]
pub enum Endianness {
    Little,
    Big,
    NA,
}

#[derive(Debug, Clone, Copy)]
pub enum DvSerErr {
    BufferTooSmall,
}

#[derive(Debug, Clone, Copy)]
pub enum DvDeErr {
    WrongBufferSize,
}

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

#[derive(DvDeSer)]
pub struct TestStruct {
    size: u32,
    value: u32,
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
