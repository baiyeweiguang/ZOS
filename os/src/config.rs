pub const KERNEL_STACK_SIZE: usize = 4096 * 2;
pub const KERNEL_HEAP_SIZE: usize = 0x30_0000;
pub const USER_STACK_SIZE: usize = 4096 * 2;
pub const PAGE_SIZE: usize = 0x1000;
pub const PAGE_SIZE_BITS: usize = 0xc; // 12

pub const MEMORY_END: usize = 0x80800000; // 8M
                                          // 跳板的地址 放在最高处
pub const TRAMPOLINE_ADDRESS: usize = usize::MAX - PAGE_SIZE + 1;
pub const TRAP_CONTEXT_ADDRESS: usize = TRAMPOLINE_ADDRESS - PAGE_SIZE;

pub const MAX_APP_NUM: usize = 5;
pub const APP_BASE_ADDRESS: usize = 0x80400000;
pub const APP_SIZE_LIMIT: usize = 0x20000;

pub use crate::board::CLOCK_FREQ;
