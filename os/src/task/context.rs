use crate::trap::trap_return;

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

    // 在初始化TASK_MANAGER时被调用
    // 传入这个task对应的内核栈指针
    pub fn goto_trap_ret(kstack_ptr: usize) -> Self {
        // 在__switch函数（先保存完TaskContext的各种寄存器后）的最后一行，会调用汇编指令ret
        // 然后CPU会跳转到ra寄存器中的地址，也就是trap_return函数
        // trap_return函数会调用trap.S的__restore函数
        // __restore函数保存TrapContext的各种寄存器后，调用汇编指令sret
        // sret会将CPU的特权级从S态切换到U态，然后跳转到TrapContext中的sepc寄存器中的地址
        // 而spec寄存器在app_init_context函数中被设置为elf文件的入口地址
        // 所以最终会跳到入口地址开始执行用户程序
        Self {
            ra: trap_return as usize,
            sp: kstack_ptr,
            s: [0; 12],
        }
    }
}
