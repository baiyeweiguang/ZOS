use alloc::sync::Arc;

use crate::loader::get_app_data_by_name;
use crate::mm::{translate_ref_mut, translate_str};
use crate::println;
use crate::task::{
    add_task, current_task, current_user_token, exit_current_and_run_next,
    suspend_current_and_run_next,
};
use crate::timer::get_time;

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    println!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

pub fn sys_get_time() -> isize {
    get_time() as isize
}

pub fn sys_fork() -> isize {
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_trap_cx = new_task.inner_exclusive_access().get_trap_cx();

    // 子进程的返回值为0
    new_trap_cx.x[10] = 0;

    let new_pid = new_task.pid.0;

    add_task(new_task);

    // 父进程的返回值为子进程的pid
    new_pid as isize
}

pub fn sys_exec(path: *const u8) -> isize {
    let token = current_user_token();
    let path = translate_str(token, path);

    if let Some(data) = get_app_data_by_name(path.as_str()) {
        // println!("try to exec {:?}", path);
        let task = current_task().unwrap();
        task.exec(data);
        0
    } else {
        -1
    }
}

pub fn sys_getpid() -> isize {
    current_task().unwrap().pid.0 as isize
}

/// 获取pid为ipid的僵尸子进程的退出码
/// 如果ipid为-1，则等待任意子进程
/// 如果不存在pid为ipid的子进程，则返回-1
/// 如果存在pid为ipid的子进程，但其不是僵尸进程（还在运行），则返回-2
pub fn sys_waitpid(ipid: isize, exit_code_ptr: *mut i32) -> isize {
    let task = current_task().unwrap();

    let mut inner = task.inner_exclusive_access();
    // 寻找是否有对应pid的子进程
    if !inner
        .children
        .iter()
        .any(|p| ipid == -1 || ipid as usize == p.getpid())
    {
        return -1;
    }

    // 寻找是否有对应pid的僵尸子进程
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        p.inner_exclusive_access().is_zombie() && (ipid == -1 || ipid as usize == p.getpid())
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // 确保离开此函数后child会被释放
        assert_eq!(Arc::strong_count(&child), 1);

        let found_pid = child.getpid();
        let exit_code = child.inner_exclusive_access().exit_code;
        // 注意！这里传入的token不能用current_user_token()函数来获得 -> de了半个小时bug的血泪
        // 因为上面我们已经borrow了inner，而current_user_token()会再次borrow，造成borrow twice崩溃
        *translate_ref_mut(inner.get_user_token(), exit_code_ptr) = exit_code;

        found_pid as isize
    } else {
        -2
    }
}

// pub fn sys_sbrk(size: i32) -> isize {
//     if let Some(old_brk) = change_program_brk(size) {
//         old_brk as isize
//     } else {
//         -1
//     }
// }
