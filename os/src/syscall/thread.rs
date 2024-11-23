/// 功能：当前进程创建一个新的线程
/// 参数：entry 表示线程的入口函数地址，arg 表示传给线程入口函数参数
/// 返回值：创建的线程的 TID
/// syscall ID: 1000
pub fn sys_thread_create(entry: usize, arg: usize) -> isize {
  0
}

pub fn sys_gettid() -> isize {
  0
} 

pub fn sys_waittid(tid: isize) -> isize {
  0
}