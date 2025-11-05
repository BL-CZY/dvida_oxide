use core::fmt;
use core::ptr::null_mut;

use limine::framebuffer::Framebuffer;
use limine::request::FramebufferRequest;
use spin::Mutex;

use super::BUILTIN_FONT;

pub struct DebugWriter {
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
    is_cursor_on: bool,
    cursor_blink_interval: u8,
    color_buffer: [[u64; 160]; 100],
    text_buffer: [[u8; 160]; 100],
}

// Debug Terminal Context
pub static mut DEFAULT_WRITER: Mutex<DebugWriter> = Mutex::new(DebugWriter {
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
    is_cursor_on: true,
    cursor_blink_interval: 2,
    color_buffer: [[0xFFFFFF00000000; 160]; 100],
    text_buffer: [[b'\0'; 160]; 100],
});

pub enum TerminalErr {
    NoFrameBuffer,
}

#[used]
#[unsafe(link_section = ".requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

impl DebugWriter {
    pub fn init_debug_terminal(&mut self) {
        if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response() {
            if let Some(framebuffer) = framebuffer_response.framebuffers().next() {
                self.configure_debug_terminal(
                    &framebuffer,
                    framebuffer.width(),
                    framebuffer.height(),
                );
            }
        }
    }

    fn configure_debug_terminal(&mut self, buffer: &Framebuffer, res_width: u64, res_height: u64) {
        // set default context
        self.frame_buffer_width = buffer.width();
        self.frame_buffer_height = buffer.height();
        self.frame_buffer_addr = buffer.addr() as *mut u32;
        self.terminal_width = res_width / 8;
        self.terminal_height = res_height / 16;

        self.clear_debug_terminal();
    }

    fn clear_debug_terminal(&mut self) {
        for row in 0..self.terminal_height as usize {
            for col in 0..self.terminal_width as usize {
                self.text_buffer[row][col] = b'\0';
                self.color_buffer[row][col] = 0xFFFFFF00000000;
                self.debug_render_char(b'\0', row as u64, col as u64);
            }
        }
        self.current_row = 0;
        self.current_col = 0;

        self.update_debug_cursor(false);
    }

    pub fn blink_debug_cursor(&mut self) {
        if self.is_cursor_on {
            if self.cursor_blink_interval == 0 {
                self.update_debug_cursor(true);
                self.is_cursor_on = false;
                self.cursor_blink_interval = 2;
            } else {
                self.cursor_blink_interval -= 1;
            }
        } else {
            if self.cursor_blink_interval == 0 {
                self.update_debug_cursor(false);
                self.is_cursor_on = true;
                self.cursor_blink_interval = 2;
            } else {
                self.cursor_blink_interval -= 1;
            }
        }
    }

    fn remove_debug_cursor(&mut self, row: u64, col: u64) {
        for i in 0..16u64 {
            for j in 0..8u64 {
                unsafe {
                    let pixel_offset: u64 = (row * 16 + i) * self.frame_buffer_width + col * 8 + j;

                    *(self.frame_buffer_addr.add(pixel_offset as usize)) = self.cur_bg_color;
                }
            }
        }
    }

    fn draw_debug_cursor(&mut self, row: u64, col: u64) {
        for i in 0..16u64 {
            for j in 0..8u64 {
                unsafe {
                    let pixel_offset: u64 = (row * 16 + i) * self.frame_buffer_width + col * 8 + j;

                    *(self.frame_buffer_addr.add(pixel_offset as usize)) = 0xFFFFFF;
                }
            }
        }
    }

    fn update_debug_cursor(&mut self, remove: bool) {
        self.remove_debug_cursor(self.cursor_row.into(), self.cursor_col.into());

        // draw the character hidden by the cursor
        self.debug_render_char(
            self.text_buffer[self.cursor_row as usize][self.cursor_col as usize],
            self.cursor_row,
            self.cursor_col,
        );

        if !remove {
            self.draw_debug_cursor(self.current_row.into(), self.current_col.into());
        }

        self.cursor_row = self.current_row;
        self.cursor_col = self.current_col;
    }

    fn debug_render_char(&mut self, character: u8, row: u64, col: u64) {
        let font_offset: usize = character as usize * 16;

        for i in 0..16u64 {
            for j in 0..8u64 {
                unsafe {
                    let pixel_offset = (row * 16 + i) * self.frame_buffer_width + col * 8 + j;
                    if ((BUILTIN_FONT[font_offset + i as usize] >> (7 - j)) & 0x1) == 0x1 {
                        *(self.frame_buffer_addr.add(pixel_offset as usize)) = self.cur_fg_color;
                    } else {
                        *(self.frame_buffer_addr.add(pixel_offset as usize)) = self.cur_bg_color;
                    }
                }
            }
        }
    }

    fn debug_render_buffer(&mut self) {
        for row in 0..self.terminal_height as usize {
            for col in 0..self.terminal_width as usize {
                self.cur_bg_color = self.color_buffer[row][col] as u32;
                self.cur_fg_color = (self.color_buffer[row][col] >> 32) as u32;
                self.debug_render_char(self.text_buffer[row][col], row as u64, col as u64);
            }
        }

        self.current_row = self.terminal_height - 1;
        self.current_col = 0;

        self.update_debug_cursor(false);
    }

    fn debug_terminal_moveup(&mut self) {
        for i in 1..(self.terminal_height as usize) {
            for j in 0..(self.terminal_width as usize) {
                self.color_buffer[i - 1][j] = self.color_buffer[i][j];
                self.text_buffer[i - 1][j] = self.text_buffer[i][j];
            }
        }

        for i in 0..(self.terminal_width as usize) {
            self.color_buffer[(self.terminal_height - 1) as usize][i] = 0xFFFFFF00000000;
            self.text_buffer[(self.terminal_height - 1) as usize][i] = 0;
        }

        self.debug_render_buffer();
    }

    fn debug_terminal_advance(&mut self) {
        self.current_col += 1;
        if self.current_col == self.terminal_width {
            self.current_col = 0;
            self.current_row += 1;
            if self.current_row == self.terminal_height {
                self.current_row = self.terminal_height - 1;
                self.debug_terminal_moveup();
            }
        }
    }

    fn debug_terminal_newline(&mut self) {
        self.current_col = 0;
        self.current_row += 1;
        if self.current_row == self.terminal_height {
            self.current_row = self.terminal_height - 1;
            self.debug_terminal_moveup();
        }
    }

    fn debug_terminal_putbyte(&mut self, byte: u8) {
        let font_offset = byte as usize * 16;

        for i in 0..16 {
            for j in 0..8 {
                let pixel_offset = (self.current_row * 16 + i) * self.frame_buffer_width
                    + self.current_col * 8
                    + j;

                if ((BUILTIN_FONT[font_offset + i as usize] >> (7 - j)) & 0x1) == 0x1 {
                    unsafe {
                        *(self.frame_buffer_addr.add(pixel_offset as usize)) = self.cur_fg_color;
                    }
                } else {
                    unsafe {
                        *(self.frame_buffer_addr.add(pixel_offset as usize)) = self.cur_bg_color;
                    }
                }
            }
        }

        self.color_buffer[self.current_row as usize][self.current_col as usize] =
            (self.cur_fg_color as u64) << 32 | (self.cur_bg_color) as u64;

        self.text_buffer[self.current_row as usize][self.current_col as usize] = byte;
        self.debug_terminal_advance();
    }

    pub fn write_string(&mut self, format: &str) {
        for byte in format.bytes() {
            match byte {
                b'\n' => self.debug_terminal_newline(),
                0x00..=0x7f => self.debug_terminal_putbyte(byte),
                _ => self.debug_terminal_putbyte(0xFE),
            }
        }
        self.update_debug_cursor(false);
    }
}

impl fmt::Write for DebugWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;
    unsafe {
        interrupts::without_interrupts(|| DEFAULT_WRITER.lock().write_fmt(args).unwrap());
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::debug::terminal::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
