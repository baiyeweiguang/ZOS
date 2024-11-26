use alloc::collections::vec_deque::VecDeque;
use alloc::sync::Arc;

use crate::task::{block_current_and_run_next, current_task, wakeup_task, TaskControlBlock};

use super::UPSafeCell;

pub struct Semaphore {
    pub inner: UPSafeCell<SemaphoreInner>,
}

pub struct SemaphoreInner {
    pub count: isize,
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl Semaphore {
    pub fn new(res_count: usize) -> Self {
        Self {
            inner: UPSafeCell::new(SemaphoreInner {
                count: res_count as isize,
                wait_queue: VecDeque::new(),
            }),
        }
    }

    /// V操作,count++,count <= 0时唤醒一个等待的任务
    pub fn up(&self) {
        let mut inner = self.inner.exclusive_access();
        inner.count += 1;
        if inner.count <= 0 {
            if let Some(task) = inner.wait_queue.pop_front() {
                wakeup_task(task);
            }
        }
    }

    /// P操作,count--,count < 0时阻塞当前任务
    pub fn down(&self) {
        let mut inner = self.inner.exclusive_access();
        inner.count -= 1;
        if inner.count < 0 {
            inner.wait_queue.push_back(current_task().unwrap());
            drop(inner);
            block_current_and_run_next();
        }
    }
}
