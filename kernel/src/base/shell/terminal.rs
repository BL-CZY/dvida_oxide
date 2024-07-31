use core::ptr::null_mut;

use limine::framebuffer::Framebuffer;

pub struct TerminalContext {
    frame_buffer_width: u64,
    frame_buffer_height: u64,
    frame_buffer_addr: *mut u8,
    terminal_width: u32,
    terminal_height: u32,
    current_row: u32,
    current_col: u32,
    cur_bg_color: u32,
    cur_fg_color: u32,
    color_buffer: [[u64; 160]; 50],
    text_buffer: [[char; 160]; 50],
}

impl TerminalContext {
    pub fn new() -> Self {
        TerminalContext {
            frame_buffer_width: 0,
            frame_buffer_height: 0,
            frame_buffer_addr: null_mut(),
            terminal_width: 0,
            terminal_height: 0,
            current_row: 0,
            current_col: 0,
            cur_bg_color: 0,
            cur_fg_color: 0,
            color_buffer: [[0xFFFFFF00000000; 160]; 50],
            text_buffer: [['\0'; 160]; 50],
        }
    }
}

pub fn terminal_init(buffer: &Framebuffer, width: u32, height: u32) -> TerminalContext {
    let mut result = TerminalContext::new();

    // set context
    result.frame_buffer_width = buffer.width();
    result.frame_buffer_height = buffer.height();
    result.frame_buffer_addr = buffer.addr();
    result.terminal_width = width / 8;
    result.terminal_height = height / 16;
    result.cur_bg_color = 0x000000;
    result.cur_fg_color = 0xFFFFFF;
    result
}

pub fn set_cursor(context: &TerminalContext, row: u32, col: u32, remove: bool) {
    let offset: u32 = row * context.terminal_width + col;

    if offset >= context.terminal_width * context.terminal_height {
        return;
    }

    if remove {}
}
