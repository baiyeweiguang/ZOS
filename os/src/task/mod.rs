mod context;
mod id;
mod manager;
mod process;
mod processor;
mod switch;
mod task;

use alloc::sync::Arc;
pub use context::TaskContext;
pub use manager::{add_task, wakeup_task};
use process::ProcessControlBlock;
// pub use task::TaskStatus;

use crate::loader::get_app_data_by_name;
use lazy_static::*;
pub use task::TaskControlBlock;

// 初始进程
lazy_static! {
    pub static ref INITPROC: Arc<ProcessControlBlock> =
        ProcessControlBlock::new(get_app_data_by_name("initproc").unwrap());
}

pub fn add_initproc() {
    add_task(INITPROC.inner_exclusive_access().get_task(0));
}

pub use processor::{
    block_current_and_run_next, current_process, current_task, current_trap_cx, current_user_token,
    exit_current_and_run_next, run_tasks, suspend_current_and_run_next,
};
