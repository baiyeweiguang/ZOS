// Uniprocessor interior mutability primitives

use core::cell::{RefCell, RefMut};

// 对一个static数据结构进行封装，让我们能够使用可变的全局变量
// 只能在单线程下使用
// RefCell中的数据是放在堆上的
pub struct UPSafeCell<T> {
  inner: RefCell<T>,
}

unsafe impl<T> Sync for UPSafeCell<T> {}

impl<T> UPSafeCell<T> {
  /// User is responsible to guarantee that inner struct is only used in
  /// uniprocessor.
  pub fn new(value: T) -> Self {
    Self {
      inner: RefCell::new(value),
    }
  }

  /// Exclusive access inner data in UPSafeCell. Panic if the data has been borrowed.
  pub fn exclusive_access(&self) -> RefMut<'_, T> {
    self.inner.borrow_mut()
  }
}

// 在rust中，当你解引用时，如果数据实现了Copy trait（i32等基础数据实现了，但是String这种类型没有），将会进行拷贝，否则将会进行move