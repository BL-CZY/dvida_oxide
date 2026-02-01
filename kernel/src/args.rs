use crate::{crypto::guid::Guid, log};
use limine::request::ExecutableCmdlineRequest;

#[used]
#[unsafe(link_section = ".requests")]
pub static EXECUTABLE_CMDLINE_REQUEST: ExecutableCmdlineRequest = ExecutableCmdlineRequest::new();

#[derive(Debug, Clone, Default)]
pub struct ArgsRes {
    pub root_drive: Guid,
    pub root_entry: Guid,
}

pub fn parse_args() -> ArgsRes {
    let args = EXECUTABLE_CMDLINE_REQUEST
        .get_response()
        .expect("No arg passed")
        .cmdline()
        .to_string_lossy();

    let args = args.split(' ').filter_map(|arg| arg.split_once('='));

    let mut res = ArgsRes::default();

    for (key, val) in args {
        log!("Received arg: {:?}={:?}", key, val);

        match key {
            "root_drive_guid" => res.root_drive = Guid::from_str(val).unwrap_or(Guid::default()),
            "root_partition_guid" => {
                res.root_entry = Guid::from_str(val).unwrap_or(Guid::default())
            }
            _ => {}
        }
    }

    res
}
