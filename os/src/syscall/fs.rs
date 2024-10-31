//! File and filesystem-related syscalls
use crate::print;

const FD_STDOUT: usize = 1;

/// write buf of length `len`  to a file with `fd`
/// return the number of bytes written
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
  match fd => {
    FD_STDOUT => {
      let slice = unsafe { core::slice::from_raw_parts(buf, len) };
      let utf8_str = core::str::from_utf8(slice).unwrap();
      print!("{}", utf8_str);

    }
    _ => {
      panic!("Unsupport file descriptor: {}", fd);
    }
  }
}