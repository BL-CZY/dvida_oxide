use alloc::string::String;
use limine::{request::KernelFileRequest, response::KernelFileResponse};
use terminal::log;

pub static KERNEL_FILE_REQUEST: KernelFileRequest = KernelFileRequest::new();

#[derive(Debug, Clone)]
pub struct ArgsRes {
    pub root_drive: usize,
    pub root_entry: usize,
}

pub fn parse_args() -> ArgsRes {
    let response: &KernelFileResponse = KERNEL_FILE_REQUEST.get_response().unwrap();

    let args = String::from_utf8_lossy(response.file().cmdline());
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
