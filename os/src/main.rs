#![no_std]
#![no_main]

use core::arch::global_asm;

// macro_use用来将console的宏导出来 下面的lang_items也能用print和println
#[macro_use]
mod sync;
mod batch;
mod console;
mod lang_items;
mod sbi;
mod syscall;
mod trap;

global_asm!(include_str!("entry.asm"));
// global_asm!(include_str!("link_app.S"));

// 默认情况 rust编译器会对每个函数进行名称修饰(name mangling) 保证每个函数都有唯一的名字 以支持重载等特性
// 使用#[no_mangle]属性修饰 可以保证rust_main在汇编语言中的标签就是rust_main
#[no_mangle]
pub fn rust_main() -> ! {
    clear_bss();
    println!("Hello wolrd");
    // panic!("Test panic");
    loop {}
}

fn clear_bss() {
    extern "C" {
        // 这些函数的ABI与C语言兼容
        fn sbss();
        fn ebss();
    }
    // sbss as uszie可以获得sbss函数的地址
    // usize: The pinter-sized integer type (docs.rust-lang.org)
    (sbss as usize..ebss as usize).for_each(|a| {
        // *: rust的裸指针 相当于C指针
        unsafe { (a as *mut u8).write_volatile(0) }
    })
}
