// user模式下的stdout

use crate::read;

use super::write;
use core::fmt::{self, Write};

struct Stdout;

const STDOUT_FD: usize = 1;
const STDIN_FD: usize = 0;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // as_bytes(): 将字符串转换为[u8]
        write(STDOUT_FD, s.as_bytes());
        Ok(())
    }
}

pub fn print(args: fmt::Arguments) {
    Stdout.write_fmt(args).unwrap();
}

pub fn getchar() -> u8 {
    let mut c = [0u8; 1];
    read(STDIN_FD, &mut c);
    c[0]
}

// #[macro_export]: 让其他模块能访问到这个宏
#[macro_export]
// ,: 这个模式用,进行分割 tt:token tree +:表示匹配一个或多个模式 ?:整个模式是可选的
macro_rules! print {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!($fmt $(, $($arg)+)?));
    }
}

#[macro_export]
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?));
    }
}
