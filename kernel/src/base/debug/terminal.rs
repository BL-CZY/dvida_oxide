use core::ptr::null_mut;

use limine::framebuffer::Framebuffer;
use limine::request::FramebufferRequest;

use super::BUILTIN_FONT;

pub struct TerminalContext {
    frame_buffer_width: u64,
    frame_buffer_height: u64,
    frame_buffer_addr: *mut u32,
    terminal_width: u64,
    terminal_height: u64,
    current_row: u64,
    current_col: u64,
    cur_bg_color: u32,
    cur_fg_color: u32,
    cursor_row: u64,
    cursor_col: u64,
    color_buffer: [[u64; 160]; 50],
    text_buffer: [[char; 160]; 50],
}

// Debug Terminal Context
static mut DTC: TerminalContext = TerminalContext {
    frame_buffer_width: 0,
    frame_buffer_height: 0,
    frame_buffer_addr: null_mut(),
    terminal_width: 0,
    terminal_height: 0,
    current_row: 0,
    current_col: 0,
    cur_bg_color: 0x000000,
    cur_fg_color: 0xFFFFFF,
    cursor_row: 0,
    cursor_col: 0,
    color_buffer: [[0xFFFFFF00000000; 160]; 50],
    text_buffer: [['\0'; 160]; 50],
};

pub enum TerminalErr {
    NoFrameBuffer,
}

#[used]
#[link_section = ".requests"]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

pub fn init_debug_terminal() {
    if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response() {
        if let Some(framebuffer) = framebuffer_response.framebuffers().next() {
            configure_debug_terminal(&framebuffer, 100, 100);
        }
    }
}

fn configure_debug_terminal(buffer: &Framebuffer, width: u64, height: u64) {
    // set default context
    unsafe {
        DTC.frame_buffer_width = buffer.width();
        DTC.frame_buffer_height = buffer.height();
        DTC.frame_buffer_addr = buffer.addr() as *mut u32;
        DTC.terminal_width = width / 8;
        DTC.terminal_height = height / 16;
    }

    clear_debug_terminal();
}

fn clear_debug_terminal() {
    unsafe {
        for row in 0..DTC.terminal_height as usize {
            for col in 0..DTC.terminal_width as usize {
                DTC.text_buffer[row][col] = '\0';
                DTC.color_buffer[row][col] = 0xFFFFFF00000000;
                debug_render_char('\0', row as u64, col as u64);
            }
        }
        DTC.current_row = 0;
        DTC.current_col = 0;
    }

    update_debug_cursor(false);
}

fn remove_debug_cursor(row: u64, col: u64) {
    for i in 0..16u64 {
        for j in 0..8u64 {
            unsafe {
                let pixel_offset: u64 = (row * 16 + i) * DTC.frame_buffer_width + col * 8 + j;

                *(DTC.frame_buffer_addr.add(pixel_offset as usize)) = DTC.cur_bg_color;
            }
        }
    }
}

fn draw_debug_cursor(row: u64, col: u64) {
    for i in 0..16u64 {
        for j in 0..8u64 {
            unsafe {
                let pixel_offset: u64 = (row * 16 + i) * DTC.frame_buffer_width + col * 8 + j;

                *(DTC.frame_buffer_addr.add(pixel_offset as usize)) = 0xFFFFFF;
            }
        }
    }
}

fn update_debug_cursor(remove: bool) {
    unsafe {
        remove_debug_cursor(DTC.cursor_row.into(), DTC.cursor_col.into());

        if !remove {
            draw_debug_cursor(DTC.current_row.into(), DTC.current_col.into());
        }

        DTC.cursor_row = DTC.current_row;
        DTC.cursor_col = DTC.current_col;
    }
}

fn debug_render_char(character: char, row: u64, col: u64) {
    let font_offset: usize = character as usize * 16;

    for i in 0..16u64 {
        for j in 0..8u64 {
            unsafe {
                let pixel_offset = (row * 16 + i) * DTC.frame_buffer_width + col * 8 + j;
                if ((BUILTIN_FONT[font_offset + i as usize] >> (7 - j)) & 0x1) == 0x1 {
                    *(DTC.frame_buffer_addr.add(pixel_offset as usize)) = DTC.cur_fg_color;
                } else {
                    *(DTC.frame_buffer_addr.add(pixel_offset as usize)) = DTC.cur_bg_color;
                }
            }
        }
    }
}

unsafe fn debug_render_buffer() {
    for row in 0..DTC.terminal_height as usize {
        for col in 0..DTC.terminal_width as usize {
            DTC.cur_bg_color = DTC.color_buffer[row][col] as u32;
            DTC.cur_fg_color = (DTC.color_buffer[row][col] >> 32) as u32;
        }
    }

    DTC.current_row = DTC.terminal_height - 1;
    DTC.current_col = 0;
    update_debug_cursor(false);
}

unsafe fn debug_terminal_moveup() {
    for i in 0..DTC.terminal_height as usize {
        for j in 0..DTC.terminal_width as usize {
            DTC.color_buffer[i - 1][j] = DTC.color_buffer[i][j];
            DTC.text_buffer[i - 1][j] = DTC.text_buffer[i][j];
        }
    }

    for i in 0..DTC.terminal_width as usize {
        DTC.color_buffer[(DTC.terminal_height - 1) as usize][i] = 0xFFFFFF00000000;
        DTC.text_buffer[(DTC.terminal_height - 1) as usize][i] = '\0';
    }

    debug_render_buffer();
}

unsafe fn debug_terminal_advance() {
    DTC.current_col += 1;
    if DTC.current_col == DTC.terminal_width {
        DTC.current_col = 0;
        DTC.current_row += 1;
        if DTC.current_row == DTC.terminal_height {
            DTC.current_row = DTC.terminal_height - 1;
            debug_terminal_moveup();
        }
    }
}

unsafe fn debug_terminal_back() {
    if DTC.current_col == 0 {
        DTC.current_col = DTC.terminal_width - 1;
        if DTC.current_row == 0 {
            DTC.current_row = 0;
        } else {
            DTC.current_row -= 1;
        }
    } else {
        DTC.current_col -= 1;
    }
}
