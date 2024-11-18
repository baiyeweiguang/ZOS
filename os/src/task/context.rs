#[derive(Copy, Clone)]
#[repr(C)]
pub struct TaskContext {
    /// return address ( e.g. __restore ) of __switch ASM function
    ra: usize,
    /// kernel stack pointer of app
    sp: usize,
    /// callee saved registers:  s 0..11
    s: [usize; 12],
}

impl TaskContext {
    // 对应原版的zero_init
    pub fn new_empty() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }

    // __restore在trap/trap.S中
    // 在初始化TASK_MANAGER时被调用
    // 传入这个task对应的内核栈指针
    // 对应原版的go_to_restore
    pub fn new_setted_trap_ret(kstack_ptr: usize) -> Self {
        extern "C" {
            fn __restore();
        }

        // 在__switch函数的最后一行，会调用汇编指令ret
        // 然后CPU会跳转到ra寄存器中的地址，也就是__restore函数，并继续往下执行
        Self {
            ra: __restore as usize,
            sp: kstack_ptr,
            s: [0; 12],
        }
    }
}
