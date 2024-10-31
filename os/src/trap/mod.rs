mod context;
use crate::syscall::syscall;
use crate::batch::run_next_app;
use context::TrapContext;
use core::f32::consts::E;
use riscv::register::{
    scause::{self, Exception::*, Interrupt, Trap},
    stval, stvec,
};

global_asm!(include_str!("trap.S"));

pub fn init() {
    extern "C" {
        fn __alltraps();
    }

    // stvec寄存器保存中断处理函数的地址，
    // 这里将中断处理函数的地址设置为__alltraps（在trap.S中用汇编实现），__alltraps负责保存
    // 中断上下文到内核栈中，并调用trap_handler进行中断分发
    unsafe {
        stvec::write(__alltraps as usize, stvec::TrapMode::Direct);
    }
}

#[no_mangle]
pub fn trap_handler(cx: &mut TrapContext) -> &mut TrapContext {
    let scause = scause::read();
    let stval = stval::read();
    match scause.cause() {
        Exception(ex) => {
            match ex {
                UserEnvCall => {
                    // spec: 当trap为异常时，sepc指向引起异常的指令
                    cx.sepc += 4;
                    cx.x[10] = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]) as usize;
                }
                StoreFault | StorePageFault => {
                  println!("[kernel] PageFault in application, kernel killed it.");
                  run_next_app();
                }
                IllegalInstruction => {
                    println!("[kernel] IllegalInstruction in application, kernel killed it.");
                    run_next_app();
                }
                _ => {
                    panic!("Unhandled exception: {:?}\n", ex);
                }
            }
        }
        Interrupt(int) => {
            match int {
                _ => {
                    panic!("Unhandled interrupt: {:?}\n", int);
                }
            }
        }
    }
    cx
}
