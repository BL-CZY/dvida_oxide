#[no_std]
#[derive(Clone, Copy, Debug)]
pub enum Endianness {
    Little,
    Big,
    NA,
}

#[derive(Debug, Clone, Copy)]
pub enum DvSerErr {}

#[derive(Debug, Clone, Copy)]
pub enum DvDeErr {}

pub trait DvSerialize {
    /// the serialize function takes in self, endianness, writes data to a slice of data
    /// return the amount of bytes written
    /// it will error if the buffer is too small
    fn serialize(&self, endianness: Endianness, target: &mut [u8]) -> Result<usize, DvSerErr>;
}

pub trait DvDeserialize {
    /// the deserialize function takes in endianness, a slice of data, and returns the parsed self
    /// it will error if the conversion goes wrong
    fn deserialize(endianness: Endianness, input: &[u8]) -> Result<Self, DvDeErr>
    where
        Self: Sized;
}
