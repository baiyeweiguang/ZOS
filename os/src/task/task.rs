use core::cell::RefMut;

use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};

use super::{
    id::{alloc_kernel_stack, pid_alloc, KernelStack, PidHandle, TaskUserRes},
    process::ProcessControlBlock,
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
    Ready,
    Running,
    Blocked,
}

pub struct TaskControlBlock {
    // immutable
    pub process: Weak<ProcessControlBlock>,
    pub kstack: KernelStack,
    // mutable
    inner: UPSafeCell<TaskControlBlockInner>,
}

pub struct TaskControlBlockInner {
    pub res: Option<TaskUserRes>,
    pub trap_cx_ppn: PhysPageNum,
    pub task_cx: TaskContext,
    pub task_status: TaskStatus,
    pub exit_code: Option<i32>,
}

impl TaskControlBlockInner {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }

    fn get_status(&self) -> TaskStatus {
        self.task_status
    }
}

impl TaskControlBlock {
    pub fn new(parent: Arc<ProcessControlBlock>, ustack_base: usize, alloc_user_res: bool) -> Self {
        let res = TaskUserRes::new(parent.clone(), ustack_base, alloc_user_res);
        let trap_cx_ppn = res.trap_cx_ppn();
        let kernel_stack = alloc_kernel_stack();
        let kernel_stack_top = kernel_stack.get_top();
        Self {
            process: Arc::downgrade(&parent),
            kstack: kernel_stack,
            inner: UPSafeCell::new(TaskControlBlockInner {
                res: Some(res),
                trap_cx_ppn,
                task_cx: TaskContext::goto_trap_ret(kernel_stack_top),
                task_status: TaskStatus::Ready,
                exit_code: None,
            }),
        }
    }

    pub fn inner_exclusive_access(&self) -> RefMut<'_, TaskControlBlockInner> {
        self.inner.exclusive_access()
    }

    pub fn get_user_token(&self) -> usize {
        let process = self.process.upgrade().unwrap();
        let inner = process.inner_exclusive_access();
        inner.memory_set.token()
    }

    pub fn getpid(&self) -> usize {
        let process = self.process.upgrade().unwrap();
        process.pid.0
    }

    pub fn gettid(&self) -> usize {
        self.inner_exclusive_access().res.as_ref().unwrap().tid
    }
}

// pub struct TaskControlBlockInner {
//     pub trap_cx_ppn: PhysPageNum,
//     #[allow(unused)]
//     pub base_size: usize,
//     pub task_cx: TaskContext,
//     pub task_status: TaskStatus,
//     pub memory_set: MemorySet,
//     pub parent: Option<Weak<TaskControlBlock>>,
//     pub children: Vec<Arc<TaskControlBlock>>,
//     pub exit_code: i32,
// }

// impl TaskControlBlockInner {
//     pub fn get_trap_cx(&self) -> &'static mut TrapContext {
//         self.trap_cx_ppn.get_mut()
//     }
//     pub fn get_user_token(&self) -> usize {
//         self.memory_set.token()
//     }
//     fn get_status(&self) -> TaskStatus {
//         self.task_status
//     }
//     pub fn is_zombie(&self) -> bool {
//         self.get_status() == TaskStatus::Zombie
//     }
// }

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

//     pub fn inner_exclusive_access(&self) -> RefMut<'_, TaskControlBlockInner> {
//         self.inner.exclusive_access()
//     }

//     pub fn getpid(&self) -> usize {
//         self.pid.0
//     }
// }
