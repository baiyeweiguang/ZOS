use alloc::{sync::Arc, vec::Vec};

use crate::task::{
    block_current_and_run_next, current_task, suspend_current_and_run_next, wakeup_task,
    TaskControlBlock,
};

use super::UPSafeCell;

// 实现Send的类型可以在线程间安全的传递其所有权
// 实现Sync的类型可以在线程间安全的共享(通过引用)
pub trait Mutex: Sync + Send {
    fn lock(&self);
    fn unlock(&self);
}

// RISC-V架构规定，从用户态陷入内核态后，所有中断默认被自动屏蔽
// 所以不需要考虑因为中断导致的问题，甚至不需要原子操作

pub struct MutexSpin {
    locked: UPSafeCell<bool>,
}

/// 自旋锁，忙等待
impl MutexSpin {
    pub fn new() -> Self {
        Self {
            locked: UPSafeCell::new(false),
        }
    }
}

impl Mutex for MutexSpin {
    fn lock(&self) {
        loop {
            let mut locked = self.locked.exclusive_access();
            if *locked {
                drop(locked);
                suspend_current_and_run_next();
                // 这个continue实际没用吧
                continue;
            } else {
                *locked = true;
            }
        }
    }

    fn unlock(&self) {
        // 这里会不会产生一个bug
        // 就是我可以强行解别人的锁再给自己上锁（每调用lock就调用unlock了）
        let mut locked = self.locked.exclusive_access();
        *locked = false;
    }
}

pub struct MutexBlocking {
    inner: UPSafeCell<MutexBlockingInner>,
}

pub struct MutexBlockingInner {
    locked: bool,
    wait_queue: Vec<Arc<TaskControlBlock>>,
}

impl MutexBlocking {
    pub fn new() -> Self {
        Self {
            inner: UPSafeCell::new(MutexBlockingInner {
                locked: false,
                wait_queue: Vec::new(),
            }),
        }
    }
}

impl Mutex for MutexBlocking {
    fn lock(&self) {
        let mut inner = self.inner.exclusive_access();
        if inner.locked {
            inner.wait_queue.push(Arc::clone(&current_task().unwrap()));
            drop(inner);
            block_current_and_run_next();
        }
    }

    fn unlock(&self) {
        let mut inner = self.inner.exclusive_access();
        assert!(inner.locked);
        if let Some(waiting_task) = inner.wait_queue.pop() {
            // 唤醒的线程访问临界区，其他的线程继续等待
            wakeup_task(waiting_task);
        } else {
            // 所有线程都访问过临界区了
            inner.locked = false;
        }
    }
}
