use crate::println;
use crate::sbi::shutdown;
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use core::arch::asm;
use lazy_static::*;

const MAX_APP_NUM: usize = 10;
const APP_BASE_ADDRESS: usize = 0x8040000;
const APP_SIZE_LIMIT: usize = 0x20000;

struct AppManager {
    num_app: usize,
    current_app: usize, // id
    app_start: [usize; MAX_APP_NUM + 1],
}

impl AppManager {
    pub fn print_app_info(&self) {
        println!("[kernel] total {} apps", self.num_app);
        for i in 0..self.num_app {
            println!(
                "[kernel] app_{} [{:#x}, {:#x}]",
                i,
                self.app_start[i],
                self.app_start[i + 1]
            );
        }
    }

    // 将app_id对应的应用加载到内存中
    pub fn load_app(&self, app_id: usize) {
        if app_id >= self.num_app {
            println!("[kernel] All applications completed!");
            shutdown(false);
        }

        println!("[kernel] Loading app_{}", app_id);

        unsafe {
            // 清理内存
            core::slice::from_raw_parts_mut(APP_BASE_ADDRESS as *mut u8, APP_SIZE_LIMIT).fill(0);

            // 加载应用
            let app_start = self.app_start[app_id];
            let app_end = self.app_start[app_id + 1];
            let app_src = core::slice::from_raw_parts(app_start as *const u8, app_end - app_start);

            let app_dst =
                core::slice::from_raw_parts_mut(APP_BASE_ADDRESS as *mut u8, app_src.len());
            app_dst.copy_from_slice(app_src);

            // Memory fence about fetching the instruction memory
            // It is guaranteed that a subsequent instruction fetch must
            // observes all previous writes to the instruction memory.
            // Therefore, fence.i must be executed after we have loaded
            // the code of the next app into the instruction memory.
            // See also: riscv non-priv spec chapter 3, 'Zifencei' extension.
            // 保证前面所有操作（加载app）执行完了，才执行下面这一行fence.i
            asm!("fence.i");

            // 注意我们在加载完应用代码之后插入了一条奇怪的汇编指令 fence.i ，它起到什么作用呢？
            // 我们知道缓存是存储层级结构中提高访存速度的很重要一环。而 CPU 对物理内存所做的缓存又分成
            // 数据缓存 (d-cache) 和 指令缓存 (i-cache) 两部分，分别在 CPU 访存和取指的时候使用。
            // 在取指的时候，对于一个指令地址， CPU 会先去 i-cache 里面看一下它是否在某个已缓存的缓存行内，
            // 如果在的话它就会直接从高速缓存中拿到指令而不是通过总线访问内存。通常情况下，
            // CPU 会认为程序的代码段不会发生变化，因此 i-cache 是一种只读缓存。
            // 但在这里，OS 将修改会被 CPU 取指的内存区域，这会使得 i-cache 中含有与内存中不一致的内容。
            // 因此， OS 在这里必须使用取指屏障指令 fence.i ，它的功能是保证
            // 在它之后的取指过程必须能够看到在它之前的所有对于取指内存区域的修改 ，这样才能保证 CPU 访问的应用代码是最新的
            // 而不是 i-cache 中过时的内容。至于硬件是如何实现 fence.i 这条指令的，这一点每个硬件的具体实现方式都可能不同，
            // 比如直接清空 i-cache 中所有内容或者标记其中某些内容不合法等等。
        }
    }

    pub fn get_current_app(&self) -> usize {
        return self.current_app;
    }

    pub fn move_to_next_app(&mut self) {
        self.current_app += 1;
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

// batch模块对外暴露的重要接口

// 因为这里会第一次使用到APP_MANAGER，所以会触发APP_MANAGER的初始化
pub fn init() {
    print_app_info();
}

pub fn print_app_info() {
    APP_MANAGER.exclusive_access().print_app_info();
}

pub fn run_next_app() -> ! {
    let mut app_manager = APP_MANAGER.exclusive_access();
    let current_app = app_manager.get_current_app();

    app_manager.load_app(current_app);

    app_manager.move_to_next_app();
    drop(app_manager);

    extern "C" {
        fn __restore(ctx_addr: usize) -> !;
    }

    unsafe {
        let ctx = TrapContext::app_init_context(APP_BASE_ADDRESS, USER_STACK.get_sp());
        __restore(KERNEL_STACK.push_context(ctx) as *const _ as usize);
    }
}

// 用户栈和内核栈

const KERNEL_STACK_SIZE: usize = 4096 * 4;
const USER_STACK_SIZE: usize = 4096 * 4;

#[repr(align(4096))]
struct KernelStack {
    data: [u8; KERNEL_STACK_SIZE],
}

impl KernelStack {
    fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + KERNEL_STACK_SIZE
    }

    pub fn push_context(&self, cx: TrapContext) -> &'static mut TrapContext {
        // 为什么这里移动栈后不修改sp??当前的get_sp()返回的是定值，有点抽象，虽然不影响结果
        let cx_ptr = (self.get_sp() - core::mem::size_of::<TrapContext>()) as *mut TrapContext;
        unsafe {
            *cx_ptr = cx;
        }
        unsafe { cx_ptr.as_mut().unwrap() }
    }
}

#[repr(align(4096))]
struct UserStack {
    data: [u8; USER_STACK_SIZE],
}

impl UserStack {
    // 返回栈顶指针，因为risc-v中栈是向下增长的
    fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + USER_STACK_SIZE
    }
}

static KERNEL_STACK: KernelStack = KernelStack {
    data: [0; KERNEL_STACK_SIZE],
};

static USER_STACK: UserStack = UserStack {
    data: [0; USER_STACK_SIZE],
};
