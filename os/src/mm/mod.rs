mod address;
mod frame_allocator;
mod heap_allocator;
mod memory_set;
mod page_table;

pub use address::{PhysPageNum, VirtAddr};
pub use memory_set::MapPermission;
pub use memory_set::MemorySet;
pub use memory_set::KERNEL_SPACE;

pub use page_table::{translate_buffer, translate_ref_mut, translate_str};

pub fn init() {
    heap_allocator::init_heap();
    frame_allocator::init_frame_allocator();
    // 开启分页机制
    KERNEL_SPACE.exclusive_access().activate();
}

pub fn alloc_kernel_stack(start_va: VirtAddr, end_va: VirtAddr) {
    KERNEL_SPACE.exclusive_access().insert_framed_area(
        start_va.into(),
        end_va.into(),
        MapPermission::R | MapPermission::W,
    );
}
