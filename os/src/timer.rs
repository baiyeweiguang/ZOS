use crate::config::CLOCK_FREQ;
use crate::sbi::set_timer;
use riscv::register::time;

const TICKS_PER_SEC: usize = 100;
const MSEC_PER_SEC: usize = 1000;

pub fn get_time() -> usize {
    time::read()
}

#[allow(unused)]
/// get current time in milliseconds
pub fn get_time_ms() -> usize {
    time::read() / (CLOCK_FREQ / MSEC_PER_SEC)
}

// risc-v 64 中有两个寄存器mtime和mtimecmp，mtime记录当前时间，如果mtime的值大于mtimecmp，就会触发一次时钟中断
// 通过设置mtimecmp的值，可以控制下一次时钟中断发生的时机
pub fn set_next_trigger() {
    set_timer(get_time() + CLOCK_FREQ / TICKS_PER_SEC);
}
