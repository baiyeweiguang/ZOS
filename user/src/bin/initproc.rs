#![no_std]
#![no_main]

use user_lib::{exec, exit, fork, wait, yield_};

#[macro_use]
extern crate user_lib;

#[no_mangle]
pub fn main() -> i32 {
    println!("INITPROC");

    if fork() == 0 {
        //   exit(0);
        // println!("Start user_shell");
        exec("user_shell\0", &[]);
    } else {
        loop {
            let mut exit_code: i32 = 0;
            let pid = wait(&mut exit_code);
            if pid == -1 {
                yield_();
                continue;
            }
            println!(
                "[initproc] Released a zombie process, pid={}, exit_code={}",
                pid, exit_code,
            );
        }
    }
    0
}
