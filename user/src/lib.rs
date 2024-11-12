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
// 系统加载后会跳到0x80400000执行_start函数
#[link_section = ".text.entry"]
pub extern "C" fn _start() -> ! {
    clear_bss();
    exit(main());
    panic!("unreachable after sys_exit!");
}

// #[linkage = "weak"]表示这个main函数为弱符号，即如果有其他同名函数，那么这个函数会被覆盖。这样保证执行的main函数是用户程序的main函数。
#[linkage = "weak"]
#[no_mangle]
fn main() -> i32 {
    panic!("Cannot find main!");
}

fn clear_bss() {
    extern "C" {
        fn start_bss();
        fn end_bss();
    }

    (start_bss as usize..end_bss as usize).for_each(|addr| unsafe {
        (addr as *mut u8).write_volatile(0);
    })
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
