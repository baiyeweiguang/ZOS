use alloc::sync::Arc;

use crate::{
    sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore},
    task::{block_current_and_run_next, current_process, current_task},
    timer::{add_timer, get_time_ms},
};

pub fn sys_sleep(ms: usize) -> isize {
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}

pub fn sys_mutex_create(blocking: bool) -> isize {
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    // id复用
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        id as isize
    } else {
        // 分配新id
        process_inner.mutex_list.push(mutex);
        process_inner.mutex_list.len() as isize - 1
    }
}

pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();

    // 它原版直接下标访问，没考虑非法访问，这里改成get()
    // let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());

    if let Some(mutex) = process_inner
        .mutex_list
        .get(mutex_id)
        .and_then(|m| m.as_ref())
    {
        let mutex = Arc::clone(mutex);
        drop(process_inner);
        drop(process);
        mutex.lock();
        0
    } else {
        -1
    }
}

pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();

    // let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());

    if let Some(mutex) = process_inner
        .mutex_list
        .get(mutex_id)
        .and_then(|m| m.as_ref())
    {
        let mutex = Arc::clone(mutex);
        drop(process_inner);
        drop(process);
        mutex.lock();
        0
    } else {
        -1
    }
}

pub fn sys_semaphore_create(res_count: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner.semaphore_list.len() - 1
    };

    id as isize
}

pub fn sys_semaphore_up(sem_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();

    if let Some(sem) = process_inner
        .semaphore_list
        .get(sem_id)
        .and_then(|s| s.as_ref())
    {
        let sem = Arc::clone(sem);
        drop(process_inner);
        drop(process);
        sem.up();
        0
    } else {
        -1
    }
}

pub fn sys_semaphore_down(sem_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();

    if let Some(sem) = process_inner
        .semaphore_list
        .get(sem_id)
        .and_then(|s| s.as_ref())
    {
        let sem = Arc::clone(sem);
        drop(process_inner);
        drop(process);
        sem.down();
        0
    } else {
        -1
    }
}

pub fn sys_condvar_create() -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}

pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();

    if let Some(condvar) = process_inner
        .condvar_list
        .get(condvar_id)
        .and_then(|c| c.as_ref())
    {
        let condvar = Arc::clone(condvar);
        drop(process_inner);
        drop(process);
        condvar.signal();
        0
    } else {
        -1
    }
}

pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();

    // 获取条件变量
    let condvar = match process_inner
        .condvar_list
        .get(condvar_id)
        .and_then(|c| c.as_ref())
    {
        Some(condvar) => Arc::clone(condvar),
        None => return -1,
    };

    // 获取互斥锁
    let mutex = match process_inner
        .mutex_list
        .get(mutex_id)
        .and_then(|m| m.as_ref())
    {
        Some(mutex) => Arc::clone(mutex),
        None => return -1,
    };

    // 释放对 process_inner 的独占访问
    drop(process_inner);
    drop(process);

    // 等待条件变量
    condvar.wait(mutex);

    0
}
