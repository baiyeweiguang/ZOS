mod context;
mod switch;
mod task;

use core::panic;

pub use context::TaskContext;
use switch::__switch;
// pub use task::TaskStatus;

use crate::config::MAX_APP_NUM;
use crate::loader::{get_num_app, init_app_cx};
use crate::println;
use crate::sbi::shutdown;
use crate::sync::UPSafeCell;
use lazy_static::*;
use task::{TaskControlBlock, TaskStatus};

/// The task manager, where all the tasks are managed.
///
/// Functions implemented on `TaskManager` deals with all task state transitions
/// and task context switching. For convenience, you can find wrappers around it
/// in the module level.
///
/// Most of `TaskManager` are hidden behind the field `inner`, to defer
/// borrowing checks to runtime. You can see examples on how to use `inner` in
/// existing functions on `TaskManager`.
pub struct TaskManager {
    /// total number of tasks
    num_app: usize,
    /// use inner value to get mutable access
    inner: UPSafeCell<TaskManagerInner>,
}

/// Inner of Task Manager
pub struct TaskManagerInner {
    /// task list
    tasks: [TaskControlBlock; MAX_APP_NUM],
    /// id of current `Running` task
    current_task: usize,
}

lazy_static! {
  /// Global variable: TASK_MANAGER
  pub static ref TASK_MANAGER: TaskManager = {
      let num_app = get_num_app();

      let mut tasks = [TaskControlBlock {
          task_cx: TaskContext::new_empty(),
          task_status: TaskStatus::UnInit,
      }; MAX_APP_NUM];

      for (i, task) in tasks.iter_mut().enumerate() {
          task.task_cx = TaskContext::new_setted_trap_ret(init_app_cx(i));
          task.task_status = TaskStatus::Ready;
      }

      TaskManager {
          num_app,
          inner: 
              UPSafeCell::new(TaskManagerInner {
                  tasks,
                  current_task: 0,
              })
          ,
      }
  };
}

impl TaskManager {
    pub fn mark_current_suspend(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Ready;
        // println!("[debug kernel current task {} ready]", current);
    }

    pub fn mark_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Exited;
        // println!("[debug kernel current task {} exited]", current);
    }

    fn run_next_task(&self) {
        if let Some(next) = self.find_next_task() {
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            let next_task_cx_ptr = &mut inner.tasks[next].task_cx as *mut TaskContext;

            inner.tasks[next].task_status = TaskStatus::Running;
            inner.current_task = next;
            drop(inner);
            unsafe {
                __switch(current_task_cx_ptr, next_task_cx_ptr);
            }
        } else {
            println!("All applications completed!");
            shutdown(false);
        }
    }

    // 从这里开始，系统从内核态进入用户态
    fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        let next_task_cx_ptr = &mut inner.tasks[0].task_cx as *mut TaskContext;

        inner.tasks[0].task_status = TaskStatus::Running;
        inner.current_task = 0;
        drop(inner);
        unsafe {
            let mut unused = TaskContext::new_empty();
            __switch(&mut unused, next_task_cx_ptr);
        }

        panic!("unreachable in run_first_task!");
    }

    fn find_next_task(&self) -> Option<usize> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;

        for i in 1..self.num_app + 1 {
            let next = (current + i) % self.num_app;
            if inner.tasks[next].task_status == TaskStatus::Ready {
                println!("[kernel] current: {}, next: {}, num_app: {}", current, next, self.num_app);
                return Some(next);
            }
        }

        None
    }
}

pub fn exit_current_and_run_next() {
    TASK_MANAGER.mark_current_exited();
    TASK_MANAGER.run_next_task();
}

pub fn suspend_current_and_run_next() {
    TASK_MANAGER.mark_current_suspend();
    TASK_MANAGER.run_next_task();
}

pub fn run_first_task() {
    TASK_MANAGER.run_first_task();
}