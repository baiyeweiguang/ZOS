use riscv::register::sstatus::{self, Sstatus, SPP};

/// Trap Context
#[repr(C)]
pub struct TrapContext {
    /// general regs[0..31]
    pub x: [usize; 32],
    /// CSR sstatus      
    /// SPP 等字段给出 Trap 发生之前 CPU 处在哪个特权级（S/U）等信息
    pub sstatus: Sstatus,
    /// CSR sepc
    /// 当 Trap 是一个异常的时候，记录 Trap 发生之前执行的最后一条指令的地址
    pub sepc: usize,
    // 下面三个字段只在初始化时赋值，不会在运行时改变
    /// 表示内核地址空间的 token ，即内核页表的起始物理地址
    pub kernel_satp: usize,
    /// 当前应用在内核地址空间中的内核栈栈顶指针的虚拟地址
    pub kernel_sp: usize,
    /// 内核中trap_handler的虚拟地址
    pub trap_handler: usize,
}

impl TrapContext {
    /// set stack pointer to x_2 reg (sp)
    pub fn set_sp(&mut self, sp: usize) {
        self.x[2] = sp;
    }

    // 开辟一个栈空间（TrapContext），设置了sstatus、sepc和sp三个寄存器，其他寄存器都初始化为0
    // 用于初始化TrapContext，构造一个用户态的默认上下文
    pub fn app_init_context(
        entry: usize,
        sp: usize,
        kernel_satp: usize,
        kernel_sp: usize,
        trap_handler: usize,
    ) -> Self {
        let mut sstatus = sstatus::read();
        sstatus.set_spp(SPP::User);

        let mut cx = Self {
            x: [0; 32],
            sstatus: sstatus,
            sepc: entry,                // 应用程序的入口地址
            kernel_satp: kernel_satp,   // 页表地址
            kernel_sp: kernel_sp,       // 内核栈的栈顶指针
            trap_handler: trap_handler, // trap_handler的地址
        };

        cx.set_sp(sp);
        cx
    }
}
