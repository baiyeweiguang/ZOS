use super::address::PhysPageNum;
use alloc::vec::Vec;
use core::fmt::{Debug, Formatter};

// 从ekernel到MEMORY_END的内存可以分配出去
trait FrameAllocator {
    fn new() -> Self;
    fn alloc(&mut self) -> Option<PhysPageNum>;
    fn dealloc(&mut self, ppn: PhysPageNum);
}

pub struct StackFrameAllocator {
    current: usize, //空闲内存的起始物理页号
    end: usize,     //空闲内存的结束物理页号
    recycled: Vec<usize>,
}

impl FrameAllocator for StackFrameAllocator {
    fn new() -> Self {
        StackFrameAllocator {
            current: 0,
            end: 0,
            recycled: Vec::new(),
        }
    }

    fn alloc(&mut self) -> Option<PhysPageNum> {
        if let Some(ppn) = self.recycled.pop() {
            return Some(PhysPageNum(ppn));
        } else {
            if self.current < self.end {
                let ppn = PhysPageNum(self.current);
                self.current += 1;
                return Some(ppn);
            }

            None
        }
    }

    fn dealloc(&mut self, ppn: PhysPageNum) {
        let ppn = ppn.0;
        // if ppn >= self.current || self.recycled.iter().find(|&x| *x == ppn).is_some() {
        // panic!("dealloc frame out of range");
        // }
        if ppn >= self.current || self.recycled.contains(&ppn) {
            panic!("dealloc frame out of range, PPN: {}", ppn);
        }

        self.recycled.push(ppn);
    }
}

impl StackFrameAllocator {
    pub fn init(&mut self, start: PhysPageNum, end: PhysPageNum) {
        self.current = start.0;
        self.end = end.0;
    }
}

use crate::{mm::address::PhysAddr, println, sync::UPSafeCell};
use lazy_static::lazy_static;
type FrameAllocatorImpl = StackFrameAllocator;
lazy_static! {
    pub static ref FRAME_ALLOCATOR: UPSafeCell<FrameAllocatorImpl> =
        UPSafeCell::new(FrameAllocatorImpl::new());
}

pub fn init_frame_allocator() {
    extern "C" {
        fn ekernel();
    }
    // 头需要上取整，尾需要下取整，细节
    FRAME_ALLOCATOR.exclusive_access().init(
        PhysAddr::from(ekernel as usize).ceil(),
        PhysAddr::from(crate::config::MEMORY_END).floor(),
    );
}

pub struct FrameTracker {
    pub ppn: PhysPageNum,
}

impl FrameTracker {
    pub fn new(ppn: PhysPageNum) -> Self {
        ppn.get_bytes_array().iter_mut().for_each(|x| *x = 0);
        FrameTracker { ppn }
    }
}

impl Drop for FrameTracker {
    fn drop(&mut self) {
        frame_dealloc(self.ppn);
    }
}

/// allocate a frame
pub fn frame_alloc() -> Option<FrameTracker> {
    FRAME_ALLOCATOR
        .exclusive_access()
        .alloc()
        .map(FrameTracker::new)
}

/// deallocate a frame
fn frame_dealloc(ppn: PhysPageNum) {
    FRAME_ALLOCATOR.exclusive_access().dealloc(ppn);
}

impl Debug for FrameTracker {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("FrameTracker({:#x})", self.ppn.0))
    }
}

#[allow(unused)]
pub fn frame_allocator_test() {
    let mut v: Vec<FrameTracker> = Vec::new();
    for i in 0..5 {
        let frame = frame_alloc().unwrap();
        println!("{:?}", frame);
        v.push(frame);
    }
    v.clear();
    for i in 0..5 {
        let frame = frame_alloc().unwrap();
        println!("{:?}", frame);
        v.push(frame);
    }
    drop(v);
    println!("frame_allocator_test passed!");
}
