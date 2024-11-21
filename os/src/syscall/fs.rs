//! File and filesystem-related syscalls
use crate::{
    mm::translate_buffer,
    print,
    sbi::console_getchar,
    task::{current_user_token, suspend_current_and_run_next},
};

const FD_STDIN: usize = 0;
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

pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    match fd {
        FD_STDIN => {
            assert_eq!(
                len, 1,
                "Only support len = 1 in sys_read(FD_STDIN, buf, len)"
            );
            let mut c: usize = 0;
            loop {
                let c = console_getchar();
                if c == 0 {
                    suspend_current_and_run_next();
                    continue;
                } else {
                    break;
                }
            }
            let ch = c as u8;
            let mut buffers = translate_buffer(current_user_token(), buf, len);
            unsafe {
                buffers[0].as_mut_ptr().write_volatile(ch);
            }
            1
        }
        _ => {
            panic!("Unsupport file descriptor: {}", fd);
        }
    }
}
