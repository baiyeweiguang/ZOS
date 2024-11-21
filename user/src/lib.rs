#![no_std]
// 让#[linkage = "weak"]生效
#![feature(linkage)]

// 引入模块
#[macro_use]
pub mod console;
mod lang_items;
mod syscall;

#[no_mangle]
// 将_start函数放到.text.entry段中
// 系统加载后会跳到0x10000执行_start函数
#[link_section = ".text.entry"]
pub extern "C" fn _start() -> ! {
    exit(main());
    panic!("unreachable after sys_exit!");
}

// #[linkage = "weak"]表示这个main函数为弱符号，即如果有其他同名函数，那么这个函数会被覆盖。这样保证执行的main函数是用户程序的main函数。
#[linkage = "weak"]
#[no_mangle]
fn main() -> i32 {
    panic!("Cannot find main!");
}

use syscall::*;

// 对sys_write的二次封装
pub fn write(fd: usize, buffer: &[u8]) -> isize {
    sys_write(fd, buffer)
}

pub fn exit(code: i32) -> isize {
    sys_exit(code)
}

pub fn yield_() -> isize {
    sys_yield()
}

pub fn get_time() -> isize {
    sys_get_time()
}

/// sbrk() increments the program's data space by increment bytes.  Calling
/// sbrk() with an increment of 0 can be used to find the current  location
/// of the program break.
pub fn sbrk(size: i32) -> isize {
    sys_sbrk(size)
}

pub fn getpid() -> isize {
    sys_getpid()
}
pub fn fork() -> isize {
    sys_fork()
}
pub fn exec(path: &str) -> isize {
    sys_exec(path)
}

/// 等待任意子进程退出，返回子进程的pid，-1表示没有子进程
pub fn wait(exit_code: &mut i32) -> isize {
    loop {
        match sys_waitpid(-1, exit_code as *mut _) {
            -2 => {
                yield_();
            }
            // -1 or a real pid
            exit_pid => return exit_pid,
        }
    }
}

/// 等待子进程pid退出，返回子进程的pid，-1表示没有子进程
pub fn waitpid(pid: usize, exit_code: &mut i32) -> isize {
    loop {
        match sys_waitpid(pid as isize, exit_code as *mut _) {
            -2 => {
                yield_();
            }
            // -1 or a real pid
            exit_pid => return exit_pid,
        }
    }
}

pub fn sleep(period_ms: usize) {
    let start = sys_get_time();
    while sys_get_time() < start + period_ms as isize {
        sys_yield();
    }
}
