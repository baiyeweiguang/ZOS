use crate::println;
use crate::sbi::shutdown;
use core::fmt::Debug;
use core::panic::PanicInfo;

#[panic_handler]
// ! 做为返回类型时，被称为never类型，表示函数永远不会返回。
fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        println!(
            "Picked at {}: {} {}",
            location.file(),
            location.line(),
            info.message()
        );
    } else {
        println!("Panicked: {}", info.message())
    }
    shutdown(true);
}

// 以下全部代码全是为了实现rust for循环语法糖

pub trait StepByOne {
    fn step(&mut self);
}

#[derive(Copy, Clone)]
pub struct SimpleRange<T>
where
    T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    left: T,
    right: T,
}

impl<T> SimpleRange<T>
where
    T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    pub fn new(start: T, end: T) -> Self {
        assert!(start <= end, "start {:?} > end {:?}!", start, end);
        Self {
            left: start,
            right: end,
        }
    }

    pub fn get_start(&self) -> T {
        self.left
    }

    pub fn get_end(&self) -> T {
        self.right
    }
}

impl<T> IntoIterator for SimpleRange<T>
where
    T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    type Item = T;
    type IntoIter = SimpleRangeIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        SimpleRangeIter::new(self.left, self.right)
    }
}

pub struct SimpleRangeIter<T>
where
    T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    current: T,
    end: T,
}

impl<T> SimpleRangeIter<T>
where
    T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    pub fn new(l: T, r: T) -> Self {
        Self { current: l, end: r }
    }
}

impl<T> Iterator for SimpleRangeIter<T>
where
    T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.end {
            let ret = self.current;
            self.current.step();
            Some(ret)
        } else {
            None
        }
    }
}
