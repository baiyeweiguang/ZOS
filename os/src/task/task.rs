use super::TaskContext;
use crate::{
    config::{kernel_stack_position, TRAP_CONTEXT_ADDRESS},
    mm::{MapPermission, MemorySet, PhysPageNum, VirtAddr, VirtPageNum, KERNEL_SPACE},
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
    pub fn new(elf_data: &[u8], app_id: usize) {
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
            task_cx: TaskContext::new_setted_trap_ret(kernel_stack_top),
            memory_set,
            trap_cx_ppn,
            base_size: user_sp,
            heap_bottom: user_sp,
            program_brk: user_sp,
        };
    }
}
