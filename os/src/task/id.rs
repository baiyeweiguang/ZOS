use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};
use lazy_static::lazy_static;

use crate::{
    config::{
        KERNEL_STACK_SIZE, PAGE_SIZE, TRAMPOLINE_ADDRESS, TRAP_CONTEXT_ADDRESS, USER_STACK_SIZE,
    },
    mm::{MapPermission, PhysPageNum, VirtAddr, KERNEL_SPACE},
    println,
    sync::UPSafeCell,
};

use super::process::{self, ProcessControlBlock};

// RAII的pid表示
pub struct PidHandle(pub usize);

impl Drop for PidHandle {
    fn drop(&mut self) {
        PID_ALLOCATOR.exclusive_access().dealloc(self.0);
    }
}
pub fn pid_alloc() -> PidHandle {
    let id = PID_ALLOCATOR.exclusive_access().alloc();
    PidHandle(id)
}

// 只有一个全局的PID_ALLOCATOR，但是每个Process都有自己的tid_allocator
lazy_static! {
    pub static ref PID_ALLOCATOR: UPSafeCell<RecycleAllocator> =
        UPSafeCell::new(RecycleAllocator::new());
}

// 这三个资源都和线程的生命周期相同，放在一起管理
pub struct TaskUserRes {
    pub tid: usize,
    /// 进程的UserStack分配的基地址
    pub ustack_base: usize,
    pub process: Weak<ProcessControlBlock>,
}

fn trap_cx_bottom_from_tid(tid: usize) -> usize {
    TRAP_CONTEXT_ADDRESS - tid * PAGE_SIZE
}

fn ustack_bottom_from_tid(ustack_base: usize, tid: usize) -> usize {
    ustack_base + tid * (PAGE_SIZE + USER_STACK_SIZE)
}

impl TaskUserRes {
    pub fn new(parent: Arc<ProcessControlBlock>, ustack_base: usize, alloc_user_res: bool) -> Self {
        let tid = parent.inner_exclusive_access().alloc_tid();

        let task_user_res = Self {
            tid,
            ustack_base,
            process: Arc::downgrade(&parent),
        };

        if alloc_user_res {
            task_user_res.alloc_user_res();
        }

        task_user_res
    }

    pub fn trap_cx_ppn(&self) -> PhysPageNum {
        let process = self.process.upgrade().unwrap();

        let trap_cx_bottom_va: VirtAddr = trap_cx_bottom_from_tid(self.tid).into();

        let ppn = process
            .inner_exclusive_access()
            .memory_set
            .translate(trap_cx_bottom_va.into())
            .unwrap()
            .ppn();
        ppn
    }

    pub fn trap_cx_user_va(&self) -> usize {
        trap_cx_bottom_from_tid(self.tid)
    }

    pub fn ustack_top(&self) -> usize {
        ustack_bottom_from_tid(self.ustack_base, self.tid) + USER_STACK_SIZE
    }

    pub fn ustack_base(&self) -> usize {
        self.ustack_base
    }

    /// 在进程地址空间中映射线程的用户栈和 Trap 上下文。
    pub fn alloc_user_res(&self) {
        let process = self.process.upgrade().unwrap();
        let mut process_inner = process.inner_exclusive_access();

        // alloc ustack
        let ustack_bottom = ustack_bottom_from_tid(self.ustack_base, self.tid);
        let ustack_top = ustack_bottom + USER_STACK_SIZE;
        process_inner.memory_set.insert_framed_area(
            ustack_bottom.into(),
            ustack_top.into(),
            MapPermission::R | MapPermission::W | MapPermission::U,
        );

        // alloc trap_cx
        let trap_cx_bottom = trap_cx_bottom_from_tid(self.tid);
        let trap_cx_top = trap_cx_bottom + PAGE_SIZE;
        process_inner.memory_set.insert_framed_area(
            trap_cx_bottom.into(),
            trap_cx_top.into(),
            MapPermission::R | MapPermission::W,
        );
    }

    fn dealloc_user_res(&self) {
        let process = self.process.upgrade().unwrap();
        let mut process_inner = process.inner_exclusive_access();

        // dealloc ustack
        let ustack_bottom_va: VirtAddr = ustack_bottom_from_tid(self.ustack_base, self.tid).into();
        process_inner
            .memory_set
            .remove_area_with_start_vpn(ustack_bottom_va.into());

        // dealloc trap_cx
        let trap_cx_bottom_va: VirtAddr = trap_cx_bottom_from_tid(self.tid).into();
        process_inner
            .memory_set
            .remove_area_with_start_vpn(trap_cx_bottom_va.into());
    }

    pub fn dealloc_tid(&self) {
        let process = self.process.upgrade().unwrap();
        let mut process_inner = process.inner_exclusive_access();
        process_inner.dealloc_tid(self.tid);
    }
}

impl Drop for TaskUserRes {
    fn drop(&mut self) {
        self.dealloc_tid();
        self.dealloc_user_res();
    }
}

/// Return (bottom, top) of a kernel stack of specific app in kernel space.
pub fn kernel_stack_position(app_id: usize) -> (usize, usize) {
    let top = TRAMPOLINE_ADDRESS - app_id * (KERNEL_STACK_SIZE + PAGE_SIZE);
    let bottom = top - KERNEL_STACK_SIZE;
    (bottom, top)
}

lazy_static! {
    pub static ref KSTACK_ALLOCATOR: UPSafeCell<RecycleAllocator> =
        UPSafeCell::new(RecycleAllocator::new());
}

// KernelStack是RAII实现的
// KernelStack对于一个程序来说，就是KERNEL_SPACE的memory_set的一个area
// 每个线程都有一个KernelStack
// KernelStack的id和tid、pid无关
pub struct KernelStack {
    id: usize,
}

pub fn alloc_kernel_stack() -> KernelStack {
    let kstack_id = KSTACK_ALLOCATOR.exclusive_access().alloc();
    let (kstack_bottom, kstack_top) = kernel_stack_position(kstack_id);
    KERNEL_SPACE.exclusive_access().insert_framed_area(
        kstack_bottom.into(),
        kstack_top.into(),
        MapPermission::R | MapPermission::W,
    );
    KernelStack { id: kstack_id }
}

impl KernelStack {
    #[allow(unused)]
    pub fn push_on_top<T>(&self, value: T) -> *mut T
    where
        // Sized: 在编译期就能确定大小的类型
        T: Sized,
    {
        let top = self.get_top();
        let ptr = (top - core::mem::size_of::<T>()) as *mut T;
        unsafe {
            *ptr = value;
        }
        ptr
    }

    pub fn get_top(&self) -> usize {
        let (_, kernel_stack_top) = kernel_stack_position(self.id);
        kernel_stack_top
    }
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        let (kernel_stack_bottom, _) = kernel_stack_position(self.id);
        let kernel_stack_bottom_va: VirtAddr = kernel_stack_bottom.into();
        KERNEL_SPACE
            .exclusive_access()
            .remove_area_with_start_vpn(kernel_stack_bottom_va.into());
    }
}

///Allocator struct
pub struct RecycleAllocator {
    current: usize,
    recycled: Vec<usize>,
}

impl RecycleAllocator {
    pub fn new() -> Self {
        Self {
            current: 0,
            recycled: Vec::new(),
        }
    }

    pub fn alloc(&mut self) -> usize {
        if let Some(id) = self.recycled.pop() {
            id
        } else {
            self.current += 1;
            self.current - 1
        }
    }

    pub fn dealloc(&mut self, id: usize) {
        assert!(id < self.current);
        assert!(
            !self.recycled.iter().any(|x| *x == id),
            "id {} has been deallocated!",
            id
        );
        self.recycled.push(id);
    }
}
