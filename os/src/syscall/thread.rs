use alloc::sync::Arc;

use crate::{
    mm::kernel_token,
    task::{add_task, current_task, TaskControlBlock},
    trap::{trap_handler, TrapContext},
};

/// 功能：当前进程创建一个新的线程
/// 参数：entry 表示线程的入口函数地址，arg 表示传给线程入口函数参数
/// 返回值：创建的线程的 TID
/// syscall ID: 1000
pub fn sys_thread_create(entry: usize, arg: usize) -> isize {
    let task = current_task().unwrap();
    let process = task.process.upgrade().unwrap();

    let new_task = Arc::new(TaskControlBlock::new(
        Arc::clone(&process),
        task.inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .ustack_base,
        true,
    ));

    add_task(Arc::clone(&new_task));

    let new_task_inner = new_task.inner_exclusive_access();
    let new_task_res = new_task_inner.res.as_ref().unwrap();
    let new_task_tid = new_task_res.tid;
    let mut process_inner = process.inner_exclusive_access();
    // add new thread to current process
    let tasks = &mut process_inner.tasks;
    while tasks.len() < new_task_tid + 1 {
        tasks.push(None);
    }
    tasks[new_task_tid] = Some(Arc::clone(&new_task));

    let new_task_trap_cx = new_task_inner.get_trap_cx();
    *new_task_trap_cx = TrapContext::app_init_context(
        entry,
        new_task_res.ustack_top(),
        kernel_token(),
        new_task.kstack.get_top(),
        trap_handler as usize,
    );
    (*new_task_trap_cx).x[10] = arg;

    0
}

pub fn sys_gettid() -> isize {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid as isize
}

/// thread does not exist, return -1
/// thread has not exited yet, return -2
/// otherwise, return thread's exit code
pub fn sys_waittid(tid: usize) -> isize {
    let task = current_task().unwrap();
    let process = task.process.upgrade().unwrap();
    let task_inner = task.inner_exclusive_access();
    let mut process_inner = process.inner_exclusive_access();

    // a thread cannot wait for itself
    if task_inner.res.as_ref().unwrap().tid == tid {
        return -1;
    }

    if let Some(waited_task) = process_inner.tasks[tid].as_ref() {
        let exit_code = waited_task.inner_exclusive_access().exit_code;
        if let Some(val) = exit_code {
            process_inner.tasks[tid] = None;
            return val as isize;
        } else {
            -2
        }
    } else {
        -1
    }
}
