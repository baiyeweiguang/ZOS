use core::arch::global_asm;

use super::TaskContext;

global_asm!(include_str!("switch.S"));

extern "C" {
    /// 将当前的处理机上下文保存在current_task_cx_ptr指向的位置
    /// 并恢复next_task_cx_ptr指向的处理机上下文
    pub fn __switch(current_task_cx_ptr: *mut TaskContext, next_task_cx_ptr: *const TaskContext);
}
