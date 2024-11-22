#![no_std]
#![no_main]

extern crate user_lib;

extern crate alloc;

use alloc::string::String;
use user_lib::{console::getchar, exec, fork, getpid, print, println, waitpid};

const LF: u8 = 0x0au8; // \n
const CR: u8 = 0x0du8; // \r
const DL: u8 = 0x7fu8; // delete
const BS: u8 = 0x08u8; // 退格 \b

#[no_mangle]
pub fn main() -> i32 {
    let mut line: String = String::new();
    print!(">> ");
    loop {
        let c = getchar();
        match c {
            LF | CR => {
                // 换行
                println!("");
                // 根据输入的命令调用对应的程序
                if !line.is_empty() {
                    line.push('\0');
                    let pid = fork();
                    if pid == 0 {
                        // 子进程
                        if exec(line.as_str()) == -1 {
                            println!("Error when executing! command = {}", line);
                            return -4;
                        }
                        unreachable!();
                    } else {
                        // 父进程
                        let mut exit_code: i32 = 0;
                        let exit_pid = waitpid(pid as usize, &mut exit_code);
                        assert_eq!(pid, exit_pid);
                        println!("Shell: Process {} exited with code {}", pid, exit_code);
                    }
                    line.clear();
                }
                print!(">> ");
            }
            BS | DL => {
                if !line.is_empty() {
                    print!("{}", BS as char);
                    print!(" ");
                    print!("{}", BS as char);
                    line.pop();
                }
            }
            _ => {
                let ch = c as char;
                print!("{}", ch);
                line.push(ch);
            }
        }
    }
}
