#![no_std]

mod numbers;

pub use dvida_serialize_macros::DvDeSer;
use thiserror::Error;

#[derive(Clone, Copy, Debug)]
pub enum Endianness {
    Little,
    Big,
    NA,
}

#[derive(Debug, Clone, Copy, Error)]
pub enum DvSerErr {
    #[error("The buffer is too small")]
    BufferTooSmall,
    #[error("Inappropriate string length, expected range: {0}, ={1}")]
    BadStringLength(usize, usize),
}

#[derive(Debug, Clone, Copy, Error)]
pub enum DvDeErr {
    #[error("The buffer's size is wrong")]
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
