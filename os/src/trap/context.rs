

/// Trap Context
#[repr(C)]
pub struct TrapContext {
    /// general regs[0..31]
    pub x: [usize; 32],
    /// CSR sstatus      
    pub sstatus: Sstatus,
    /// CSR sepc
    pub sepc: usize,
}

impl TrapContext {
  /// set stack pointer to x_2 reg (sp)
  pub fn set_sp(&mut self, sp: usize) {
    self.x[2] = sp;
  }

  pub fn init_context() {
    
  }
}