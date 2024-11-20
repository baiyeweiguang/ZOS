use crate::{sync::UPSafeCell, trap::TrapContext};
use alloc::sync::Arc;
use lazy_static::lazy_static;

use super::{
    manager::{add_task, fetch_task},
    switch::__switch,
    task::{TaskControlBlock, TaskStatus},
    TaskContext,
};

pub struct Processor {
    /// 当前正在运行的任务
    current: Option<Arc<TaskControlBlock>>,
    /// 当前处理器idle控制流的任务上下文
    idle_task_cx: TaskContext,
}

impl Processor {
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::new_empty(),
        }
    }

    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        // take() 方法取出 Option 中的值，留下一个 None
        self.current.take()
    }

    /// 返回当前任务的一份拷贝
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(|t| Arc::clone(t))
    }

    pub fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }
}

// 目前只有单处理器
lazy_static! {
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe { UPSafeCell::new(Processor::new()) };
}

pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

pub fn current_user_token() -> usize {
    let task = current_task().expect("no current task");
    let token = task.inner_exclusive_access().get_user_token();
    token
}

pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .get_trap_cx()
}

// 当一个进程耗尽了时间片后，内核会调用这个函数将当前处理器切换到idle进程上
// 接着这个处理器会在调用run_tasks()函数时，从idle进程切换到下一个进程
/// 换出进程，上下文保存在switched_task_cx_ptr中
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}

// 如果存在下一个任务，处理器就从idle进程切换到下一个任务
/// 换入进程
pub fn run_tasks() {
    loop {
        let mut processor = PROCESSOR.exclusive_access();
        if let Some(next_task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            let mut next_task_inner = next_task.inner_exclusive_access();
            let next_task_cx_ptr = &next_task_inner.task_cx as *const TaskContext;
            next_task_inner.task_status = TaskStatus::Running;

            // 这里必须手动释放资源，因为调用__switch函数后，CPU会被切换出去，编译器的生命周期检查会出问题
            drop(next_task_inner);
            processor.current = Some(next_task);

            unsafe { __switch(idle_task_cx_ptr, next_task_cx_ptr) };
        }
    }
}

pub fn suspend_current_and_run_next() {
    let current_task = current_task().expect("no current task");

    let mut current_task_inner = current_task.inner_exclusive_access();
    current_task_inner.task_status = TaskStatus::Ready;

    let current_task_cx_ptr = &mut current_task_inner.task_cx as *mut TaskContext;

    // 因为schedule会调用__switch，所以这里必须手动释放资源
    drop(current_task_inner);

    add_task(current_task.clone());
    schedule(current_task_cx_ptr);
}
