use crate::println;
use lazy_static::*;
use crate::sync::UPSafeCell;

const MAX_APP_NUM: usize = 10;
struct AppManager {
  num_app: usize,
  current_app: usize,
  app_start: [usize; MAX_APP_NUM + 1],
}

impl AppManager {
  pub fn print_app_info(&self) {
    println!("[kernel] total {} apps", self.num_app);
    for i in 0..self.num_app {
      println!(
        "[kernel] app_{} [{:#x}, {:#x})",
        i,
        self.app_start[i],
        self.app_start[i + 1]
      );
    }
  }
}

// lazy_static!宏让我们能够定义在堆上的全局静态变量
lazy_static! {
  // 定义了一个全局的AppManager
  static ref APP_MANAGER: UPSafeCell<AppManager> = UPSafeCell::new(
    {
      extern "C" {
        fn _num_app();
      }
      let num_app_ptr = _num_app as usize as *const usize;
      let num_app = unsafe { num_app_ptr.read_volatile()};

      let mut app_start: [usize; MAX_APP_NUM + 1] = [0; MAX_APP_NUM + 1];
      app_start[..=num_app].copy_from_slice(
        unsafe {
          core::slice::from_raw_parts(num_app_ptr.add(1) as *const usize, num_app + 1)
        }
      );

      AppManager{
        num_app,
        current_app: 0,
        app_start,
      }
    }
  );
}



