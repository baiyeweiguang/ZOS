const SYSCALL_READ: usize = 63;
const SYSCALL_WRITE: usize = 64;
const SYSCALL_EXIT: usize = 93;
const SYSCALL_YIELD: usize = 124;
// const SYSCALL_KILL: usize = 129;
const SYSCALL_GET_TIME: usize = 169;
// const SYSCALL_SBRK: usize = 214;
const SYSCALL_GETPID: usize = 172;
const SYSCALL_FORK: usize = 220;
const SYSCALL_EXEC: usize = 221;
const SYSCALL_WAITPID: usize = 260;
const SYSCALL_THREAD_CREATE: usize = 1000;
const SYSCALL_GETTID: usize = 1001;
const SYSCALL_WAITTID: usize = 1002;

mod fs;
mod process;
mod thread;

use fs::*;
use process::{sys_exec, sys_exit, sys_fork, sys_get_time, sys_getpid, sys_waitpid, sys_yield};
use thread::{sys_gettid, sys_thread_create, sys_waittid};

pub fn syscall(syscall_id: usize, args: [usize; 3]) -> isize {
    match syscall_id {
        SYSCALL_READ => sys_read(args[0], args[1] as *mut u8, args[2]),
        SYSCALL_WRITE => sys_write(args[0], args[1] as *const u8, args[2]),
        SYSCALL_EXIT => sys_exit(args[0] as i32),
        SYSCALL_YIELD => sys_yield(),
        // SYSCALL_KILL => sys_kill(args[0], args[1] as u32),
        SYSCALL_GET_TIME => sys_get_time(),
        SYSCALL_FORK => sys_fork(),
        SYSCALL_EXEC => sys_exec(args[0] as *const u8, args[1] as *const usize),
        SYSCALL_GETPID => sys_getpid(),
        SYSCALL_WAITPID => sys_waitpid(args[0] as isize, args[1] as *mut i32),
        SYSCALL_THREAD_CREATE => sys_thread_create(args[0], args[1]),
        SYSCALL_GETTID => sys_gettid(),
        SYSCALL_WAITTID => sys_waittid(args[0]) as isize,
        // SYSCALL_SBRK => sys_sbrk(args[0] as i32),
        _ => {
            panic!("Unsupported syscall_id: {}", syscall_id);
        }
    }
}
