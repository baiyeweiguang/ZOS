use core::arch::asm;

// usize可以存放指针
fn syscall(id: usize, args: [usize; 3]) -> isize {
  let mut ret: isize;
  unsafe {
      asm!(
          "ecall",
          inlateout("x10") args[0] => ret,  // x10(a0) = args[0], ret = x10(a0) 
          in("x11") args[1],  // x11(a1) = args[1]
          in("x12") args[2],  // x12(a2) = args[2]
          in("x17") id     // x17(a7) = id
      );
  }
  ret
}

const SYSCALL_WRITE: usize = 64;
const SYSCALL_EXIT: usize = 93;

// &[u8]是一个切片，是一个fat pointer，包含了指针和长度
pub fn sys_write(fd: usize, buffer: &[u8]) -> isize {
  syscall(SYSCALL_WRITE, [fd, buffer.as_ptr() as usize, buffer.len()])
}

pub fn sys_exit(exit_code: i32) -> isize {
  syscall(SYSCALL_EXIT, [exit_code as usize, 0, 0])
}