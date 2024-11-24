use core::cell::RefMut;

use alloc::{
    string::String,
    sync::{Arc, Weak},
    vec::Vec,
};

use crate::{
    config::TRAP_CONTEXT_ADDRESS,
    mm::{translate_ref_mut, MemorySet, VirtAddr, KERNEL_SPACE},
    print, println,
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
    pub children: Vec<Arc<ProcessControlBlock>>,
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

        // println!("try to new pcb");
        // 分配pid
        let pid_handle = pid_alloc();

        // 创建进程控制块
        let process = Arc::new(Self {
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
        let kstack_top = main_task.kstack.get_top();
        drop(task_inner);
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            ustack_top,
            KERNEL_SPACE.exclusive_access().token(),
            kstack_top,
            trap_handler as usize,
        );

        // 将主线程加入进程的任务列表
        let mut process_inner = process.inner_exclusive_access();
        process_inner.tasks.push(Some(Arc::clone(&main_task)));
        drop(process_inner);
        insert_into_pid2process(process.getpid(), Arc::clone(&process));

        add_task(main_task);
        process
    }

    pub fn fork(self: &Arc<Self>) -> Arc<Self> {
        let mut parent_inner = self.inner_exclusive_access();

        // 只支持单线程
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

        parent_inner.children.push(Arc::clone(&child));

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
            trap_cx.kernel_sp = child_main_task.kstack.get_top();
        }

        insert_into_pid2process(child.getpid(), Arc::clone(&child));

        // add this thread to scheduler
        add_task(child_main_task);
        child
    }

    pub fn exec(self: &Arc<Self>, elf_data: &[u8], args: Vec<String>) {
        assert_eq!(self.inner_exclusive_access().thread_count(), 1);

        let (memory_set, ustack_base, entry_point) = MemorySet::from_elf(elf_data);
        let new_token = memory_set.token();

        // 更换地址空间
        self.inner_exclusive_access().memory_set = memory_set;

        // 因为地址空间变化，需要重新为主线程分配资源
        let task = self.inner_exclusive_access().get_task(0);
        let mut task_inner = task.inner_exclusive_access();
        task_inner.res.as_mut().unwrap().ustack_base = ustack_base;
        task_inner.res.as_mut().unwrap().alloc_user_res();
        task_inner.trap_cx_ppn = task_inner.res.as_mut().unwrap().trap_cx_ppn();

        // 将参数压入栈中
        // 假如有2个参数
        // 用户栈的布局如下
        // | argc | &argv[0] | &argv[1] | argv[0] | argv[1] |
        let mut user_sp = task_inner.res.as_ref().unwrap().ustack_top();

        // 为argc和argv分配空间
        user_sp -= (args.len() + 1) * core::mem::size_of::<usize>();
        let argv_base = user_sp;
        let mut argv: Vec<_> = (0..=args.len())
            .map(|i| {
                translate_ref_mut(
                    new_token,
                    (argv_base + i * core::mem::size_of::<usize>()) as *mut usize,
                )
            })
            .collect();

        for i in 0..args.len() {
            user_sp -= args[i].len() + 1;
            *argv[i] = user_sp;
            // argv后面的才是数据
            let mut p = user_sp;
            for c in args[i].as_bytes() {
                *translate_ref_mut(new_token, p as *mut u8) = *c;
                p += 1;
            }
            *translate_ref_mut(new_token, p as *mut u8) = 0;
        }

        // 保证user_sp按8B对齐(K210规定)
        user_sp -= user_sp % core::mem::size_of::<usize>();

        // 修改TrapContext
        let mut trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            task.kstack.get_top(),
            trap_handler as usize,
        );
        trap_cx.x[10] = args.len(); // argc
        trap_cx.x[11] = argv_base; // argv
        *task_inner.get_trap_cx() = trap_cx;
    }

    pub fn getpid(&self) -> usize {
        self.pid.0
    }

    pub fn is_zombie(&self) -> bool {
        self.inner_exclusive_access().is_zombie
    }
}
