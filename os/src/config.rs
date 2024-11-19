pub const KERNEL_STACK_SIZE: usize = 4096 * 2;
pub const KERNEL_HEAP_SIZE: usize = 0x30_0000;
pub const USER_STACK_SIZE: usize = 4096 * 2;
pub const PAGE_SIZE: usize = 0x1000;
pub const PAGE_SIZE_BITS: usize = 0xc; // 12

pub const MEMORY_END: usize = 0x80800000; // 8M

// 跳板的地址 放在最高处
// TRAPOLINE_ADDRESS和TRAP_CONTEXT_ADDRESS都是在应用虚拟空间中
// 内核需要访问的话可以通过调用task::current_trap_cx获得可变引用进行访问
pub const TRAMPOLINE_ADDRESS: usize = usize::MAX - PAGE_SIZE + 1;
pub const TRAP_CONTEXT_ADDRESS: usize = TRAMPOLINE_ADDRESS - PAGE_SIZE;

// pub const MAX_APP_NUM: usize = 5;
pub const APP_BASE_ADDRESS: usize = 0x80400000;
pub const APP_SIZE_LIMIT: usize = 0x20000;

pub use crate::board::CLOCK_FREQ;

/// Return (bottom, top) of a kernel stack of specific app in kernel space.
pub fn kernel_stack_position(app_id: usize) -> (usize, usize) {
    let top = TRAMPOLINE_ADDRESS - app_id * (KERNEL_STACK_SIZE + PAGE_SIZE);
    let bottom = top - KERNEL_STACK_SIZE;
    (bottom, top)
}
