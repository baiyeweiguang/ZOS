use riscv::register::scause::Trap;

use super::TaskContext;
use crate::{
    config::{kernel_stack_position, TRAP_CONTEXT_ADDRESS},
    mm::{MapPermission, MemorySet, PhysPageNum, VirtAddr, VirtPageNum, KERNEL_SPACE},
    trap::{trap_handler, TrapContext},
};

// 通过 #[derive(...)] 可以让编译器为你的类型提供一些 Trait 的默认实现。
// 实现了 Clone Trait 之后就可以调用 clone 函数完成拷贝；
// 实现了 PartialEq Trait 之后就可以使用 == 运算符比较该类型的两个实例，从逻辑上说只有两个相等的应用执行状态才会被判为相等，而事实上也确实如此。
// Copy 是一个标记 Trait，决定该类型在按值传参/赋值的时候采用移动语义还是复制语义。
// #[derive(Copy, Clone)]
#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    UnInit,
    Ready,
    Running,
    Exited,
}

pub struct TaskControlBlock {
    pub task_status: TaskStatus,
    pub task_cx: TaskContext,
    pub memory_set: MemorySet,
    pub trap_cx_ppn: PhysPageNum,
    // 统计应用数据的大小
    pub base_size: usize,
    pub heap_bottom: usize,
    pub program_brk: usize,
}

impl TaskControlBlock {
    pub fn new(elf_data: &[u8], app_id: usize) -> Self {
        // 解析elf文件
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        // 为TrapContext预留的空间
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT_ADDRESS).into())
            .unwrap()
            .ppn();

        let task_status = TaskStatus::Ready;

        // 映射当前应用的内核栈
        let (kernel_stack_bottom, kernel_stack_top) = kernel_stack_position(app_id);
        KERNEL_SPACE.exclusive_access().insert_framed_area(
            kernel_stack_bottom.into(),
            kernel_stack_top.into(),
            MapPermission::R | MapPermission::W,
        );

        let task_control_block = Self {
            task_status,
            task_cx: TaskContext::goto_trap_ret(kernel_stack_top),
            memory_set,
            trap_cx_ppn,
            base_size: user_sp,
            heap_bottom: user_sp,
            program_brk: user_sp,
        };

        // 准备TrapContext
        // 这里的trap_cx是已经存在于物理内存上的
        let trap_cx = task_control_block.get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );

        task_control_block
    }

    pub fn change_program_brk(&mut self, size: i32) -> Option<usize> {
        let old_brk = self.program_brk;
        // size可能为负数
        let new_brk: isize = self.program_brk as isize + size as isize;

        // 小于heap_bottom的话，新的brk会侵犯到stack甚至其他地方的空间，不合法
        if new_brk < self.heap_bottom as isize {
            return None;
        }

        let result = if size < 0 {
            self.memory_set.shrink_to(
                VirtAddr::from(self.heap_bottom),
                VirtAddr::from(new_brk as usize),
            )
        } else {
            // 源码里是from(self.heep_bottom)，有待商榷
            self.memory_set.append_to(
                VirtAddr::from(self.program_brk),
                VirtAddr::from(new_brk as usize),
            )
        };

        if result {
            self.program_brk = new_brk as usize;
            Some(old_brk)
        } else {
            None
        }
    }

    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }

    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }
}
