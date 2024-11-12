//! Loading user applications into memory
//!
//! For chapter 3, user applications are simply part of the data included in the
//! kernel binary, so we only need to copy them to the space allocated for each
//! app to load them. We also allocate fixed spaces for each task's
//! [`KernelStack`] and [`UserStack`].

use crate::config::*;
use crate::trap::TrapContext;
use core::arch::asm;

#[repr(align(4096))]
#[derive(Copy, Clone)]
struct KernelStack {
    data: [u8; KERNEL_STACK_SIZE],
}

#[repr(align(4096))]
#[derive(Copy, Clone)]
struct UserStack {
    data: [u8; USER_STACK_SIZE],
}

static KERNEL_STACK: [KernelStack; MAX_APP_NUM] = [KernelStack {
    data: [0; KERNEL_STACK_SIZE],
}; MAX_APP_NUM];

static USER_STACK: [UserStack; MAX_APP_NUM] = [UserStack {
    data: [0; USER_STACK_SIZE],
}; MAX_APP_NUM];

impl KernelStack {
    fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + KERNEL_STACK_SIZE
    }
    pub fn push_context(&self, trap_cx: TrapContext) -> usize {
        let trap_cx_ptr = (self.get_sp() - core::mem::size_of::<TrapContext>()) as *mut TrapContext;
        unsafe {
            *trap_cx_ptr = trap_cx;
        }
        trap_cx_ptr as usize
    }
}

impl UserStack {
    fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + USER_STACK_SIZE
    }
}

/// Get base address of app i.
fn get_base_i(app_id: usize) -> usize {
    APP_BASE_ADDRESS + app_id * APP_SIZE_LIMIT
}

pub fn get_num_app() -> usize {
    extern "C" {
        fn _num_app();
    }

    unsafe { (_num_app as usize as *const usize).read_volatile() }
}

pub fn load_apps() {
    extern "C" {
        fn _num_app();
    }

    let num_app = get_num_app();

    let num_app_addr = _num_app as usize as *const usize;
    let app_start_addrs = unsafe { core::slice::from_raw_parts(num_app_addr.add(1), num_app + 1) };

    for i in 0..num_app {
        let dst_base = get_base_i(i);

        // clear dst memory
        unsafe {
            let region = core::slice::from_raw_parts_mut(dst_base as *mut u8, APP_SIZE_LIMIT);
            region.fill(0);
        }

        let src = unsafe {
            core::slice::from_raw_parts(
                app_start_addrs[i] as *const usize,
                app_start_addrs[i + 1] - app_start_addrs[i],
            )
        };

        // copy to dst
        let dst = unsafe { core::slice::from_raw_parts_mut(dst_base as *mut usize, src.len()) };
        dst.copy_from_slice(src);
    }

    unsafe {
        asm!("fence.i");
    }
}

/// get app info with entry and sp and save `TrapContext` in kernel stack
/// return the sp of `TrapContext` in kernel stack
/// 对应原版的init_app_cx
pub fn init_app_cx(app_id: usize) -> usize {
    let trap_cx =
        TrapContext::new_setted_app_entry(get_base_i(app_id), USER_STACK[app_id].get_sp());
    KERNEL_STACK[app_id].push_context(trap_cx)
}
