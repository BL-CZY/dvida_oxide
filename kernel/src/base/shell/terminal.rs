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
    current_row: u32,
    current_col: u32,
    cur_bg_color: u32,
    cur_fg_color: u32,
    cursor_row: u32,
    cursor_col: u32,
    color_buffer: [[u64; 160]; 50],
    text_buffer: [[char; 160]; 50],
}

static mut DEFAULT_TERMINAL_CONTEXT: TerminalContext = TerminalContext {
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

pub fn terminal_init_default() {
    if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response() {
        if let Some(framebuffer) = framebuffer_response.framebuffers().next() {
            terminal_configure_default(&framebuffer, 100, 100);
        }
    }
}

fn terminal_configure_default(buffer: &Framebuffer, width: u64, height: u64) {
    // set default context
    unsafe {
        DEFAULT_TERMINAL_CONTEXT.frame_buffer_width = buffer.width();
        DEFAULT_TERMINAL_CONTEXT.frame_buffer_height = buffer.height();
        DEFAULT_TERMINAL_CONTEXT.frame_buffer_addr = buffer.addr() as *mut u32;
        DEFAULT_TERMINAL_CONTEXT.terminal_width = width / 8;
        DEFAULT_TERMINAL_CONTEXT.terminal_height = height / 16;
    }

    terminal_clear_default();
}

fn terminal_clear_default() {
    unsafe {
        for row in 0..DEFAULT_TERMINAL_CONTEXT.terminal_height as usize {
            for col in 0..DEFAULT_TERMINAL_CONTEXT.terminal_width as usize {
                DEFAULT_TERMINAL_CONTEXT.text_buffer[row][col] = '\0';
                DEFAULT_TERMINAL_CONTEXT.color_buffer[row][col] = 0xFFFFFF00000000;
                render_char_default('\0', row as u64, col as u64);
            }
        }
        DEFAULT_TERMINAL_CONTEXT.current_row = 0;
        DEFAULT_TERMINAL_CONTEXT.current_col = 0;
    }

    update_cursor_default(false);
}

fn remove_cursor_default(row: u64, col: u64) {
    for i in 0..16u64 {
        for j in 0..8u64 {
            unsafe {
                let pixel_offset: u64 =
                    (row * 16 + i) * DEFAULT_TERMINAL_CONTEXT.frame_buffer_width + col * 8 + j;

                *(DEFAULT_TERMINAL_CONTEXT
                    .frame_buffer_addr
                    .add(pixel_offset as usize)) = DEFAULT_TERMINAL_CONTEXT.cur_bg_color;
            }
        }
    }
}

fn draw_cursor_default(row: u64, col: u64) {
    for i in 0..16u64 {
        for j in 0..8u64 {
            unsafe {
                let pixel_offset: u64 =
                    (row * 16 + i) * DEFAULT_TERMINAL_CONTEXT.frame_buffer_width + col * 8 + j;

                *(DEFAULT_TERMINAL_CONTEXT
                    .frame_buffer_addr
                    .add(pixel_offset as usize)) = 0xFFFFFF;
            }
        }
    }
}

fn update_cursor_default(remove: bool) {
    unsafe {
        remove_cursor_default(
            DEFAULT_TERMINAL_CONTEXT.cursor_row.into(),
            DEFAULT_TERMINAL_CONTEXT.cursor_col.into(),
        );

        if !remove {
            draw_cursor_default(
                DEFAULT_TERMINAL_CONTEXT.current_row.into(),
                DEFAULT_TERMINAL_CONTEXT.current_col.into(),
            );
        }

        DEFAULT_TERMINAL_CONTEXT.cursor_row = DEFAULT_TERMINAL_CONTEXT.current_row;
        DEFAULT_TERMINAL_CONTEXT.cursor_col = DEFAULT_TERMINAL_CONTEXT.current_col;
    }
}

fn render_char_default(character: char, row: u64, col: u64) {
    let font_offset: usize = character as usize * 16;

    for i in 0..16u64 {
        for j in 0..8u64 {
            unsafe {
                let pixel_offset =
                    (row * 16 + i) * DEFAULT_TERMINAL_CONTEXT.frame_buffer_width + col * 8 + j;
                if ((BUILTIN_FONT[font_offset + i as usize] >> (7 - j)) & 0x1) == 0x1 {
                    *(DEFAULT_TERMINAL_CONTEXT
                        .frame_buffer_addr
                        .add(pixel_offset as usize)) = DEFAULT_TERMINAL_CONTEXT.cur_fg_color;
                } else {
                    *(DEFAULT_TERMINAL_CONTEXT
                        .frame_buffer_addr
                        .add(pixel_offset as usize)) = DEFAULT_TERMINAL_CONTEXT.cur_bg_color;
                }
            }
        }
    }
}

fn render_buffer_default() {}
