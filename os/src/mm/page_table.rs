use bitflags::*;

use super::address::PhysPageNum;
use super::address::PPN_WIDTH_SV39;

bitflags! {
    pub struct PTEFlags: u8 {
        const V = 1 << 0; // Valid
        const R = 1 << 1; // Read 
        const W = 1 << 2; // Write
        const X = 1 << 3; // Execute
        const U = 1 << 4; // User,控制索引到这个页表项的对应虚拟页面是否在 CPU 处于 U 特权级的情况下是否被允许访问；
        const G = 1 << 5; // Gloabl
        const A = 1 << 6; // Accessed,处理器记录自从页表项上的这一位被清零之后，页表项的对应虚拟页面是否被访问过；
        const D = 1 << 7; // Dirty,处理器记录自从页表项上的这一位被清零之后，页表项的对应虚拟页面是否被修改过。
    }
}

/// 页表项
#[derive(Clone, Copy)]
#[repr(C)]
pub struct PageTableEntry {
  pub bits: usize,
}

impl PageTableEntry {
  pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
    Self {
      bits: (ppn.0 << 10) | flags.bits as usize,
    }
  }

  pub fn new_empty() -> Self {
    Self {
      bits: 0,
    }
  }

  pub fn ppn(&self) -> PhysPageNum {
    // (1usize << PPN_WIDTH_SV39) - 1 : 取低 PPN_WIDTH_SV39 位
    ((self.bits >> 10) & ((1usize << PPN_WIDTH_SV39) - 1)).into()
  }

  pub fn flags(&self) -> PTEFlags {
    // from_bits_truncate会自动无视前面的ppn
    PTEFlags::from_bits_truncate(self.bits as u8)
  }

  pub fn is_valid(&self) -> bool {
    (self.flags() & PTEFlags::V) != PTEFlags::empty()
  }

  pub fn is_dirty(&self) -> bool {
    (self.flags() & PTEFlags::D) != PTEFlags::empty()
  }

  pub fn is_accessed(&self) -> bool {
    (self.flags() & PTEFlags::A) != PTEFlags::empty()
  }
}


