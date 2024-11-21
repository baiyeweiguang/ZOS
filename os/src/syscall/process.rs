use crate::loader::get_app_data_by_name;
use crate::mm::translate_str;
use crate::println;
use crate::task::{
    add_task, current_task, current_user_token, exit_current_and_run_next,
    suspend_current_and_run_next,
};
use crate::timer::get_time;

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    println!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
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
        let task = current_task().unwrap();
        task.exec(data);
        0
    } else {
        -1
    }
}

// pub fn sys_sbrk(size: i32) -> isize {
//     if let Some(old_brk) = change_program_brk(size) {
//         old_brk as isize
//     } else {
//         -1
//     }
// }
