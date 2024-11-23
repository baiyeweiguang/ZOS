use core::cell::RefMut;

use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};

use crate::{
    config::TRAP_CONTEXT_ADDRESS,
    mm::{MemorySet, VirtAddr, KERNEL_SPACE},
    sync::UPSafeCell,
    task::add_task,
    trap::{trap_handler, TrapContext},
};

use super::{
    id::{pid_alloc, PidHandle, RecycleAllocator},
    manager::insert_into_pid2process,
    task::TaskControlBlock,
};

pub struct ProcessControlBlock {
    pub pid: PidHandle,
    inner: UPSafeCell<ProcessControlBlockInner>,
}

pub struct ProcessControlBlockInner {
    pub is_zombie: bool,
    pub memory_set: MemorySet,
    pub parent: Option<Weak<ProcessControlBlock>>,
    pub children: Vec<Weak<ProcessControlBlock>>,
    pub exit_code: i32,
    pub tasks: Vec<Option<Arc<TaskControlBlock>>>,
    pub task_res_allocator: RecycleAllocator,
}

impl ProcessControlBlockInner {
    pub fn alloc_tid(&mut self) -> usize {
        self.task_res_allocator.alloc()
    }

    pub fn dealloc_tid(&mut self, tid: usize) {
        self.task_res_allocator.dealloc(tid);
    }

    pub fn thread_count(&self) -> usize {
        self.tasks.len()
    }

    pub fn get_task(&self, tid: usize) -> Arc<TaskControlBlock> {
        self.tasks[tid].as_ref().unwrap().clone()
    }
}

impl ProcessControlBlock {
    pub fn inner_exclusive_access(&self) -> RefMut<'_, ProcessControlBlockInner> {
        self.inner.exclusive_access()
    }

    pub fn new(elf_data: &[u8]) -> Arc<Self> {
        // 解析elf文件
        let (memory_set, ustack_base, entry_point) = MemorySet::from_elf(elf_data);

        // println!("current app id: {}, memory usage: {}KB", app_id, user_sp / 1024);
        // 分配pid
        let pid_handle = pid_alloc();

        // 创建进程控制块
        let process = Arc::new(ProcessControlBlock {
            pid: pid_handle,
            inner: UPSafeCell::new(ProcessControlBlockInner {
                is_zombie: false,
                memory_set,
                parent: None,
                children: Vec::new(),
                exit_code: 0,
                tasks: Vec::new(),
                task_res_allocator: RecycleAllocator::new(),
            }),
        });

        // 创建主线程
        let main_task = Arc::new(TaskControlBlock::new(
            Arc::clone(&process),
            ustack_base,
            true,
        ));

        // 准备TrapContext
        let task_inner = main_task.inner_exclusive_access();
        let trap_cx = task_inner.get_trap_cx();
        let ustack_top = task_inner.res.as_ref().unwrap().ustack_top();
        let kstack_top = main_task.kernel_stack.get_top();
        drop(task_inner);
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            ustack_base,
            KERNEL_SPACE.exclusive_access().token(),
            kstack_top,
            trap_handler as usize,
        );

        // 将主线程加入进程的任务列表
        let mut process_inner = process.inner_exclusive_access();
        process_inner.tasks.push(Some(Arc::clone(&main_task)));
        drop(process_inner);
        insert_into_pid2process(process.getpid(), Arc::clone(&process));

        process
    }

    pub fn fork(self: &Arc<Self>) -> Arc<Self> {
        let mut parent_inner = self.inner_exclusive_access();

        assert_eq!(parent_inner.thread_count(), 1);

        // 创建新进程
        let new_memory_set = MemorySet::from_existed_user(&parent_inner.memory_set);
        let new_pid_handle = pid_alloc();

        let child = Arc::new(Self {
            pid: new_pid_handle,
            inner: {
                UPSafeCell::new(ProcessControlBlockInner {
                    is_zombie: false,
                    memory_set: new_memory_set,
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: 0,
                    tasks: Vec::new(),
                    task_res_allocator: RecycleAllocator::new(),
                })
            },
        });

        parent_inner.children.push(Arc::downgrade(&child));

        let ustack_base = parent_inner
            .get_task(0)
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .ustack_base();
        let child_main_task = Arc::new(TaskControlBlock::new(
            Arc::clone(&child),
            ustack_base,
            false,
        ));

        child
            .inner_exclusive_access()
            .tasks
            .push(Some(Arc::clone(&child_main_task)));

        // modify kstack_top in trap_cx of this thread
        {
            let inner = child_main_task.inner_exclusive_access();
            // 在MemorySet::from_existed_user()中已经将父进程的数据复制了一份，
            // 所以这里的new_trap_cx_ppn是已经复制了父进程的了，跟new()中的空数据不一样
            let trap_cx = inner.get_trap_cx();
            trap_cx.kernel_sp = child_main_task.kernel_stack.get_top();
        }

        insert_into_pid2process(child.getpid(), Arc::clone(&child));

        // add this thread to scheduler
        add_task(child_main_task);
        child
    }

    pub fn getpid(&self) -> usize {
        self.pid.0
    }
}
