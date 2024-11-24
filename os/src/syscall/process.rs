use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::loader::get_app_data_by_name;
use crate::mm::{translate_ref, translate_ref_mut, translate_str};
use crate::println;
use crate::task::{
    add_task, current_process, current_task, current_user_token, exit_current_and_run_next,
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

// !
pub fn sys_fork() -> isize {
    // 创建新进程
    let process = current_process();
    let new_process = process.fork();
    let new_pid = new_process.getpid();
    // 修改trap context
    let new_trap_cx = new_process
        .inner_exclusive_access()
        .get_task(0)
        .inner_exclusive_access()
        .get_trap_cx();

    // 子进程的返回值为0
    new_trap_cx.x[10] = 0;

    // 父进程的返回值为子进程的pid
    new_pid as isize
}

pub fn sys_exec(path: *const u8, mut args: *const usize) -> isize {
    let token = current_user_token();
    let path = translate_str(token, path);
    let mut args_vec: Vec<String> = Vec::new();
    
    // 支持args的话，user_shell要大改，暂时先不弄了
    // loop {
    //     let arg_str_ptr = *translate_ref(token, args);
    //     if arg_str_ptr == 0 {
    //         break;
    //     }
    //     args_vec.push(translate_str(token, arg_str_ptr as *const u8));
    //     unsafe {
    //         args = args.add(1);
    //     }
    // }

    if let Some(data) = get_app_data_by_name(path.as_str()) {
        let process = current_process();
        let argc = args_vec.len();
        process.exec(data, args_vec);
        // println!("try to exec {:?}", path);
        argc as isize
    } else {
        -1
    }
}

pub fn sys_getpid() -> isize {
    current_process().getpid() as isize
}

/// 获取pid为ipid的僵尸子进程的退出码
/// 如果ipid为-1，则等待任意子进程
/// 如果不存在pid为ipid的子进程，则返回-1
/// 如果存在pid为ipid的子进程，但其不是僵尸进程（还在运行），则返回-2
pub fn sys_waitpid(ipid: isize, exit_code_ptr: *mut i32) -> isize {
    let process = current_process();

    let mut inner = process.inner_exclusive_access();
    // 寻找是否有对应pid的子进程
    if !inner
        .children
        .iter()
        .any(|p| ipid == -1 || p.getpid() as isize == ipid)
    {
        return -1;
    }

    // 寻找是否有对应pid的僵尸子进程
    let pair = inner
        .children
        .iter()
        .enumerate()
        .find(|(_, p)| p.is_zombie() && (ipid == -1 || ipid as usize == p.getpid()));
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // 确保离开此函数后child会被释放
        assert_eq!(Arc::strong_count(&child), 1);

        let found_pid = child.getpid();
        let exit_code = child.inner_exclusive_access().exit_code;
        // 注意！这里传入的token不能用current_user_token()函数来获得 -> de了半个小时bug的血泪
        // 因为上面我们已经borrow了inner，而current_user_token()会再次borrow，造成borrow twice崩溃
        *translate_ref_mut(inner.memory_set.token(), exit_code_ptr) = exit_code;

        found_pid as isize
    } else {
        -2
    }
}

// pub fn sys_kill(pid: usize, signal: u32) -> isize {
//     if let Some(process) = pid2process(pid) {
//         if let Some(flag) = SignalFlags::from_bits(signal) {
//             process.inner_exclusive_access().signals |= flag;
//             0
//         } else {
//             -1
//         }
//     } else {
//         -1
//     }
// }

// pub fn sys_sbrk(size: i32) -> isize {
//     if let Some(old_brk) = change_program_brk(size) {
//         old_brk as isize
//     } else {
//         -1
//     }
// }
