mod context;
mod manager;
mod pid;
mod processor;
mod switch;
mod task;

use core::panic;

use alloc::sync::Arc;
use alloc::vec::Vec;
pub use context::TaskContext;
pub use manager::add_task;
use switch::__switch;
// pub use task::TaskStatus;

use crate::loader::get_app_data;
use crate::loader::get_app_data_by_name;
use crate::loader::get_num_app;
use crate::println;
use crate::sbi::shutdown;
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use lazy_static::*;
use task::{TaskControlBlock, TaskStatus};

/// 初始进程
lazy_static! {
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new(TaskControlBlock::new(
        get_app_data_by_name("initproc").unwrap()
    ));
}

pub fn add_initproc() {
    add_task(INITPROC.clone());
}

// impl TaskManager {
//     pub fn mark_current_suspend(&self) {
//         let mut inner = self.inner.exclusive_access();
//         let current = inner.current_task;
//         inner.tasks[current].task_status = TaskStatus::Ready;
//         // println!("[debug kernel current task {} ready]", current);
//     }

//     pub fn mark_current_exited(&self) {
//         let mut inner = self.inner.exclusive_access();
//         let current = inner.current_task;
//         inner.tasks[current].task_status = TaskStatus::Exited;
//         // println!("[debug kernel current task {} exited]", current);
//     }

//     fn run_next_task(&self) {
//         if let Some(next) = self.find_next_task() {
//             let mut inner = self.inner.exclusive_access();
//             let current = inner.current_task;
//             let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
//             let next_task_cx_ptr = &mut inner.tasks[next].task_cx as *const TaskContext;

//             inner.tasks[next].task_status = TaskStatus::Running;
//             inner.current_task = next;
//             drop(inner);
//             unsafe {
//                 __switch(current_task_cx_ptr, next_task_cx_ptr);
//             }
//         } else {
//             println!("All applications completed!");
//             shutdown(false);
//         }
//     }

//     // 从这里开始，系统从内核态进入用户态
//     // 调用链为run_first_task->__switch->trap_return->__restore->UserCode
//     // 在trap_return函数中，我们设置了stvec为TRAMPOLINE_ADDRESS，相当于设置了中断处理函数的入口地址
//     fn run_first_task(&self) -> ! {
//         println!("run_first_task");
//         let mut inner = self.inner.exclusive_access();
//         let next_task = &mut inner.tasks[0];
//         next_task.task_status = TaskStatus::Running;
//         let next_task_cx_ptr = &next_task.task_cx as *const TaskContext;
//         drop(inner);
//         let mut _unused = TaskContext::new_empty();
//         // before this, we should drop local variables that must be dropped manually
//         unsafe {
//             __switch(&mut _unused as *mut _, next_task_cx_ptr);
//         }
//         panic!("unreachable in run_first_task!");
//     }

//     fn find_next_task(&self) -> Option<usize> {
//         let inner = self.inner.exclusive_access();
//         let current = inner.current_task;

//         for i in 1..self.num_app + 1 {
//             let next = (current + i) % self.num_app;
//             if inner.tasks[next].task_status == TaskStatus::Ready {
//                 println!(
//                     "[kernel] current: {}, next: {}, num_app: {}",
//                     current, next, self.num_app
//                 );
//                 return Some(next);
//             }
//         }

//         None
//     }

//     fn get_current_token(&self) -> usize {
//         let inner = self.inner.exclusive_access();
//         inner.tasks[inner.current_task].get_user_token()
//     }

//     #[allow(unused)]
//     fn get_current_trap_cx(&self) -> &'static mut TrapContext {
//         let inner = self.inner.exclusive_access();
//         inner.tasks[inner.current_task].get_trap_cx()
//     }

//     pub fn change_program_brk(&self, size: i32) -> Option<usize> {
//         let mut inner = self.inner.exclusive_access();
//         let i = inner.current_task;
//         inner.tasks[i].change_program_brk(size)
//     }
// }

pub use processor::{
    current_task, current_trap_cx, current_user_token, exit_current_and_run_next,
    suspend_current_and_run_next,
};
