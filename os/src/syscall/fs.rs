//! File and filesystem-related syscalls
use crate::{mm::translate_buffer, print, task::current_user_token};

const FD_STDOUT: usize = 1;

/// write buf of length `len`  to a file with `fd`
/// return the number of bytes written
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    match fd {
        FD_STDOUT => {
            let translated_pages = translate_buffer(current_user_token(), buf, len);

            for slice in translated_pages {
                let utf8_str = core::str::from_utf8(slice).unwrap();
                print!("{}", utf8_str);
            }

            len as isize
        }
        _ => {
            panic!("Unsupport file descriptor: {}", fd);
        }
    }
}
