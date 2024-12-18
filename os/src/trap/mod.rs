mod context;
pub use context::TrapContext;
use riscv::register::sie;

use crate::config::{TRAMPOLINE_ADDRESS, TRAP_CONTEXT_ADDRESS};
use crate::task::{
    current_task, current_trap_cx, current_user_token, suspend_current_and_run_next,
};
use crate::timer::check_timer;
use crate::{task::exit_current_and_run_next, timer::set_next_trigger};
// use crate::batch::run_next_app;
use crate::println;
use crate::syscall::syscall;
use core::arch::{asm, global_asm};
use core::panic;
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Interrupt, Trap},
    stval, stvec,
};

global_asm!(include_str!("trap.S"));

pub fn init() {
    set_kernel_trap_entry();

    // extern "C" {
    // fn __alltraps();
    // }

    // unsafe {
    //     stvec::write(__alltraps as usize, stvec::TrapMode::Direct);
    // }
}

/// timer interrupt enabled
pub fn enable_timer_interrupt() {
    unsafe {
        // sie寄存器用于控制时钟中断是否开启
        sie::set_stimer();
    }
}

#[no_mangle]
pub fn trap_handler() -> ! {
    set_kernel_trap_entry();
    let scause = scause::read();
    let stval = stval::read();
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            let mut cx = current_trap_cx();
            // 这样返回到用户态的时候，会从ecall的下一个指令开始执行
            cx.sepc += 4;
            let result = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]) as usize;
            // 对于部分系统调用，比如sys_exec，调用后trap_cx会失效，所以需要重新获得一遍
            cx = current_trap_cx();
            // cx.x[10]为a0，保存返回值
            cx.x[10] = result;
        }
        Trap::Exception(Exception::StoreFault)
        | Trap::Exception(Exception::StorePageFault)
        | Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::LoadPageFault) => {
            println!("[kernel] PageFault in application, bad addr = {:#x}, bad instruction = {:#x}, kernel killed it.", stval, current_trap_cx().sepc);
            exit_current_and_run_next(-2);
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            println!("[kernel] IllegalInstruction in application, kernel killed it.");
            exit_current_and_run_next(-3);
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            // 定时器中断
            set_next_trigger();
            check_timer();
            suspend_current_and_run_next();
        }
        _ => {
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval
            );
        }
    }
    trap_return();
}

fn set_kernel_trap_entry() {
    unsafe {
        // stvec寄存器保存中断处理函数的地址，
        stvec::write(trap_from_kernel as usize, TrapMode::Direct);
    }
}

fn set_user_trap_entry() {
    // 这里将中断处理函数的地址设置为__alltraps（在trap.S中用汇编实现）
    // __alltraps负责保存中断上下文到内核栈中，并调用trap_handler进行中断分发
    unsafe {
        // 注意！ 在开启分页机制后，内核不能直接通过编译器在链接时看到的__alltraps函数对应的虚拟地址
        // 但是在trap.S中，__alltraps被放在了.text.trampoline section中，
        // 而其对应的地址符号strampoline已经被我们固定映射到了TRAMPOLINE_ADDRESS
        // 所以我们能通过跳板页面来来实际取得__alltraps和下面的__restore的汇编代码
        stvec::write(TRAMPOLINE_ADDRESS as usize, TrapMode::Direct);
    }
}

#[no_mangle]
fn trap_from_kernel() {
    let scause = scause::read();
    let stval = stval::read();
    panic!(
        "a trap from kernel! scause: {:?}, stval: {:#x}",
        scause.cause(),
        stval
    );
}

#[no_mangle]
pub fn trap_return() -> ! {
    // 让应用在U->S时，可以跳转到__alltraps
    set_user_trap_entry();
    let trap_cx_ptr = TRAP_CONTEXT_ADDRESS;
    let user_satp = current_user_token();
    extern "C" {
        fn __alltraps();
        fn __restore();
    }
    let restore_va = __restore as usize - __alltraps as usize + TRAMPOLINE_ADDRESS;

    // 调用__restore
    // 在执行第一个程序的时候，这里充当了进入U态的入口
    unsafe {
        asm!(
            "fence.i",
            "jr {restore_va}",
            restore_va = in(reg) restore_va,
            in("a0") trap_cx_ptr,
            in("a1") user_satp,
            options(noreturn)
        );
    }
    // panic!("Unreachable in back_to_user!");
}
