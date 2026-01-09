use crate::hal::fs::HalFsIOErr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i64)]
pub enum ErrNo {
    OperationNotPermitted = -0x1,
    NoSuchFileOrDirectory = -0x2,
    InputOrOutputErr = -0x3,
    BadFd = -0x9,
    PermissionDenied = -0xd,
    FileExists = -0x11,
    NotADirectory = -0x14,
    IsADirectory = -0x15,
    InvalidArgument = -0x16,
    TooManyOpenFiles = -0x18,
    NoSpaceLeft = -0x1c,
    OperationNotSupported = -0x2d,
    DirectoryNotEmpty = -0x42,
}

impl From<HalFsIOErr> for ErrNo {
    fn from(value: HalFsIOErr) -> Self {
        match value {
            HalFsIOErr::HalErr(_)
            | HalFsIOErr::DeserializationErr(_)
            | HalFsIOErr::Internal
            | HalFsIOErr::SerializationErr(_)
            | HalFsIOErr::FileTooLarge
            | HalFsIOErr::Corrupted => Self::InputOrOutputErr,

            HalFsIOErr::BadPath | HalFsIOErr::NameTooLong | HalFsIOErr::NoSuchFileOrDirectory => {
                Self::NoSuchFileOrDirectory
            }

            HalFsIOErr::FileExists => Self::FileExists,
            HalFsIOErr::DirectoryNotEmpty => Self::DirectoryNotEmpty,
            HalFsIOErr::NoPermsProvided => Self::OperationNotPermitted,
            HalFsIOErr::BufTooSmall => Self::InvalidArgument,
            HalFsIOErr::IsDirectory => Self::IsADirectory,
            HalFsIOErr::NoSpaceLeft | HalFsIOErr::NoAvailableInode => Self::NoSpaceLeft,
            HalFsIOErr::NotADirectory => Self::NotADirectory,
            HalFsIOErr::Unsupported => Self::OperationNotSupported,
        }
    }
}
