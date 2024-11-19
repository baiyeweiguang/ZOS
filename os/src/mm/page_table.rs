use alloc::vec;
use alloc::vec::Vec;

use bitflags::*;
use riscv::addr::page;

use crate::config::PAGE_SIZE;
use crate::lang_items::StepByOne;

use super::address::PhysPageNum;
use super::address::VirtPageNum;
use super::address::PPN_WIDTH_SV39;
use super::frame_allocator::frame_alloc;
use super::frame_allocator::FrameTracker;
use super::VirtAddr;

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
        Self { bits: 0 }
    }

    // 从一个页表项中获取物理页号
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

    pub fn writable(&self) -> bool {
        (self.flags() & PTEFlags::W) != PTEFlags::empty()
    }

    pub fn readable(&self) -> bool {
        (self.flags() & PTEFlags::R) != PTEFlags::empty()
    }

    pub fn executable(&self) -> bool {
        (self.flags() & PTEFlags::X) != PTEFlags::empty()
    }
}

pub struct PageTable {
    // 对应原版的root_ppn
    root_table_ppn: PhysPageNum,
    frames: Vec<FrameTracker>,
}

impl PageTable {
    pub fn new() -> Self {
        let frame = frame_alloc().unwrap();
        PageTable {
            root_table_ppn: frame.ppn,
            frames: vec![frame],
        }
    }

    // 插入一个虚拟页号到物理页号的映射（即创建/修改一个页表项）
    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        let pte = self.find_pte_create(vpn).unwrap();
        assert!(!pte.is_valid(), "vpn {:?} is mapped before mapping", vpn);
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }

    // 删除一个虚拟页号的映射
    pub fn unmap(&mut self, vpn: VirtPageNum) {
        let pte = self.find_pte(vpn).unwrap();
        assert!(
            pte.is_valid(),
            "vpn {:?} is not mapped before unmapping",
            vpn
        );
        *pte = PageTableEntry::new_empty();
    }

    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.find_pte(vpn).map(|pte| pte.clone())
    }

    pub fn token(&self) -> usize {
        // 左边 0b1000 << 60 将satp的MODE字段设置为8 表示启用Sv39模式
        // 右边 将根页表所在物理页号写到satp中
        0b1000usize << 60 | self.root_table_ppn.0
    }

    /// Temporarily used to get arguments from user space.
    pub fn from_token(satp: usize) -> Self {
        // satp: 用于控制分页机制的CSR
        Self {
            root_table_ppn: PhysPageNum::from(satp & ((1usize << 44) - 1)),
            frames: Vec::new(),
        }
    }

    // 获取一个虚拟页号对应的物理页号，如果不存在会自动创建
    fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idx = vpn.indexes();

        let mut table_ppn = self.root_table_ppn;
        for i in 0..3 {
            let pte = &mut table_ppn.get_pte_array()[idx[i]];
            if i == 2 {
                return Some(pte);
            }
            if !pte.is_valid() {
                // 为当前的页表，分配一个新的物理页
                let frame = frame_alloc().unwrap();
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                self.frames.push(frame);
            }
            table_ppn = pte.ppn();
        }

        // 理论上不会执行到这里
        None
    }

    // 获取一个虚拟页号对应的物理页号，如果不存在则返回None
    fn find_pte(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idx = vpn.indexes();

        let mut table_ppn = self.root_table_ppn;
        for i in 0..3 {
            let pte = &mut table_ppn.get_pte_array()[idx[i]];
            if i == 2 {
                return Some(pte);
            }
            if !pte.is_valid() {
                return None;
            }
            table_ppn = pte.ppn();
        }

        None
    }
}

/// 将一段缓冲区（连续的虚拟地址）翻译成若干个对应的物理页号
/// 返回时自动转化为可直接访问的若干个[u8]
pub fn translate_buffer(token: usize, ptr: *const u8, len: usize) -> Vec<&'static [u8]> {
    let mut v: Vec<&[u8]> = Vec::new();
    let page_table = PageTable::from_token(token);

    let mut start = ptr as usize;
    let end = start + len;
    while start < end {
        // 这里没考虑start是否对齐
        let start_va = VirtAddr::from(start);
        let mut vpn = start_va.floor();
        let ppn = page_table.translate(vpn).unwrap().ppn();
        vpn.step();

        let mut end_va: VirtAddr = vpn.into();

        end_va = end_va.min(VirtAddr::from(end));

        if end_va.page_offset() == 0 {
            // 此时end_va是下一页的起始地址
            v.push(&ppn.get_bytes_array()[start_va.page_offset()..]);
        } else {
            // 此时end_va和start_va在同一页
            v.push(&ppn.get_bytes_array()[start_va.page_offset()..end_va.page_offset()]);
        }

        start = end_va.into();
    }

    v
}
