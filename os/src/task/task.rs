use core::cell::RefMut;

use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};

use super::{
    pid::{pid_alloc, KernelStack, PidHandle},
    TaskContext,
};
use crate::{
    config::TRAP_CONTEXT_ADDRESS,
    mm::{MapPermission, MemorySet, PhysPageNum, VirtAddr, KERNEL_SPACE},
    println,
    sync::UPSafeCell,
    trap::{trap_handler, TrapContext},
};

// 通过 #[derive(...)] 可以让编译器为你的类型提供一些 Trait 的默认实现。
// 实现了 Clone Trait 之后就可以调用 clone 函数完成拷贝；
// 实现了 PartialEq Trait 之后就可以使用 == 运算符比较该类型的两个实例，从逻辑上说只有两个相等的应用执行状态才会被判为相等，而事实上也确实如此。
// Copy 是一个标记 Trait，决定该类型在按值传参/赋值的时候采用移动语义还是复制语义。
// #[derive(Copy, Clone)]
#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    #[allow(dead_code)]
    UnInit,
    Ready,
    Running,
    Zombie,
    Exited,
}

pub struct TaskControlBlock {
    // immutable
    pub pid: PidHandle,
    pub kernel_stack: KernelStack,
    // mutable
    // 类似于linux的tss_struct
    inner: UPSafeCell<TaskControlBlockInner>,
}

pub struct TaskControlBlockInner {
    pub trap_cx_ppn: PhysPageNum,
    #[allow(unused)]
    pub base_size: usize,
    pub task_cx: TaskContext,
    pub task_status: TaskStatus,
    pub memory_set: MemorySet,
    pub parent: Option<Weak<TaskControlBlock>>,
    pub children: Vec<Arc<TaskControlBlock>>,
    pub exit_code: i32,
}

impl TaskControlBlockInner {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }
    fn get_status(&self) -> TaskStatus {
        self.task_status
    }
    pub fn is_zombie(&self) -> bool {
        self.get_status() == TaskStatus::Zombie
    }
}

impl TaskControlBlock {
    pub fn new(elf_data: &[u8]) -> Self {
        // 解析elf文件
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);

        // println!("current app id: {}, memory usage: {}KB", app_id, user_sp / 1024);

        // 为TrapContext预留的空间
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT_ADDRESS).into())
            .unwrap()
            .ppn();

        let pid_handle = pid_alloc();
        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.get_top();

        let task_control_block = Self {
            pid: pid_handle,
            kernel_stack,
            inner: UPSafeCell::new(TaskControlBlockInner {
                trap_cx_ppn,
                base_size: user_sp,
                task_cx: TaskContext::goto_trap_ret(kernel_stack_top),
                task_status: TaskStatus::Ready,
                memory_set,
                parent: None,
                children: Vec::new(),
                exit_code: 0,
            }),
        };

        // 准备TrapContext
        // 这里的trap_cx是已经存在于物理内存上的
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        task_control_block
    }

    pub fn fork(self: &Arc<Self>) -> Arc<Self> {
        let mut parent_inner = self.inner_exclusive_access();

        // 创建新进程
        let new_memory_set = MemorySet::from_existed_user(&parent_inner.memory_set);
        // 在MemorySet::from_existed_user()中已经将父进程的数据复制了一份，
        // 所以这里的new_trap_cx_ppn是已经复制了父进程的了，跟new()中的空数据不一样
        let new_trap_cx_ppn = new_memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT_ADDRESS).into())
            .unwrap()
            .ppn();
        let new_pid_handle = pid_alloc();
        let new_kernel_stack = KernelStack::new(&new_pid_handle);
        let new_kernel_stack_top = new_kernel_stack.get_top();

        let new_task_control_block = Arc::new(TaskControlBlock {
            pid: new_pid_handle,
            kernel_stack: new_kernel_stack,
            inner: UPSafeCell::new(TaskControlBlockInner {
                trap_cx_ppn: new_trap_cx_ppn,
                base_size: parent_inner.base_size, // 继承父进程的base_size
                task_cx: TaskContext::goto_trap_ret(new_kernel_stack_top),
                task_status: TaskStatus::Ready,
                memory_set: new_memory_set,
                parent: Some(Arc::downgrade(self)),
                children: Vec::new(),
                exit_code: 0,
            }),
        });
        // 添加child
        parent_inner.children.push(new_task_control_block.clone());

        let trap_cx = new_task_control_block
            .inner_exclusive_access()
            .get_trap_cx();
        // 其他字段与父进程保持一致
        trap_cx.kernel_sp = new_kernel_stack_top;

        new_task_control_block
    }

    /// 加载elf文件，替换掉当前进程的代码和数据，并开始执行
    pub fn exec(&self, elf_data: &[u8]) {
        let (new_memory_set, new_user_sp, new_entry_point) = MemorySet::from_elf(elf_data);

        let new_trap_cx_ppn = new_memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT_ADDRESS).into())
            .unwrap()
            .ppn();

        let mut inner = self.inner_exclusive_access();
        inner.memory_set = new_memory_set;
        inner.trap_cx_ppn = new_trap_cx_ppn;

        // 因为内核栈至于pid有关，而程序的pid没有改变，所以可以直接用原来的内核栈

        // 因为当前程序还在执行中，不涉及到上下文切换
        // 所以task_cx、task_status都不需要动
        let new_trap_cx = inner.get_trap_cx();
        *new_trap_cx = TrapContext::app_init_context(
            new_entry_point,
            new_user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            self.kernel_stack.get_top(),
            trap_handler as usize,
        )
    }

    // pub fn change_program_brk(&mut self, size: i32) -> Option<usize> {
    //     let old_brk = self.program_brk;
    //     // size可能为负数
    //     let new_brk: isize = self.program_brk as isize + size as isize;

    //     // 小于heap_bottom的话，新的brk会侵犯到stack甚至其他地方的空间，不合法
    //     if new_brk < self.heap_bottom as isize {
    //         return None;
    //     }

    //     let result = if size < 0 {
    //         self.memory_set.shrink_to(
    //             VirtAddr::from(self.heap_bottom),
    //             VirtAddr::from(new_brk as usize),
    //         )
    //     } else {
    //         // 源码里是from(self.heep_bottom)，有待商榷
    //         self.memory_set.append_to(
    //             VirtAddr::from(self.program_brk),
    //             VirtAddr::from(new_brk as usize),
    //         )
    //     };

    //     if result {
    //         self.program_brk = new_brk as usize;
    //         Some(old_brk)
    //     } else {
    //         None
    //     }
    // }

    pub fn inner_exclusive_access(&self) -> RefMut<'_, TaskControlBlockInner> {
        self.inner.exclusive_access()
    }
    
    pub fn getpid(&self) -> usize {
        self.pid.0
    }
}
