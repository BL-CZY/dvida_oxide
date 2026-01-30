use crate::log;
use limine::{request::ExecutableCmdlineRequest, response::ExecutableCmdlineResponse};

pub static EXECUTABLE_CMDLINE_REQUEST: ExecutableCmdlineRequest = ExecutableCmdlineRequest::new();

#[derive(Debug, Clone)]
pub struct ArgsRes {
    pub root_drive: usize,
    pub root_entry: usize,
}

pub fn parse_args() -> ArgsRes {
    let response: &ExecutableCmdlineResponse = EXECUTABLE_CMDLINE_REQUEST.get_response().unwrap();
    let args = response.cmdline().to_str().unwrap_or_default();
    log!("Received cmdline: {}", args);

    let mut root_drive = None;
    let mut root_entry = None;

    for arg in args.split(' ') {
        match arg.split_once('=') {
            Some((var, val)) => match var {
                "root_drive" => {
                    root_drive = Some(
                        val.parse::<usize>()
                            .expect("Failed to parse root_drive index"),
                    );
                }
                "root_entry" => {
                    root_entry = Some(
                        val.parse::<usize>()
                            .expect("Failed to parse root_entry index"),
                    );
                }
                _ => {}
            },
            None => {}
        }
    }

    if root_drive.is_none() || root_entry.is_none() {
        panic!("root_drive or root_entry not provided");
    }

    ArgsRes {
        root_drive: root_drive.unwrap(),
        root_entry: root_entry.unwrap(),
    }
}
