use core::panic;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use bitflags::*;

use crate::lang_items::StepByOne;
use crate::println;

use super::address::PhysAddr;
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
            bits: ppn.0 << 10 | flags.bits as usize,
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
        PTEFlags::from_bits(self.bits as u8).unwrap()
    }

    pub fn is_valid(&self) -> bool {
        (self.flags() & PTEFlags::V) != PTEFlags::empty()
    }

    pub fn readable(&self) -> bool {
        (self.flags() & PTEFlags::R) != PTEFlags::empty()
    }

    pub fn writable(&self) -> bool {
        (self.flags() & PTEFlags::W) != PTEFlags::empty()
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
        self.find_pte(vpn).map(|pte| *pte)
    }

    pub fn translate_va(&self, va: VirtAddr) -> Option<PhysAddr> {
        self.find_pte(va.clone().floor()).map(|pte| {
            //println!("translate_va:va = {:?}", va);
            let aligned_pa: PhysAddr = pte.ppn().into();
            //println!("translate_va:pa_align = {:?}", aligned_pa);
            let offset = va.page_offset();
            let aligned_pa_usize: usize = aligned_pa.into();
            (aligned_pa_usize + offset).into()
        })
    }

    pub fn token(&self) -> usize {
        // 左边 0b1000 << 60 将satp的MODE字段设置为8 表示启用Sv39模式
        // 右边 将根页表所在物理页号写到satp中
        8usize << 60 | self.root_table_ppn.0
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
        let idxs = vpn.indexes();
        let mut ppn = self.root_table_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &mut ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                let frame = frame_alloc().unwrap();
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                self.frames.push(frame);
            }
            ppn = pte.ppn();
        }
        result
    }

    // 获取一个虚拟页号对应的物理页号，如果不存在则返回None
    fn find_pte(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_table_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &mut ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                return None;
            }
            ppn = pte.ppn();
        }
        result
    }
}

/// 将一段缓冲区（连续的虚拟地址）翻译成若干个对应的物理页号
/// 返回时自动转化为可直接访问的若干个[u8]
pub fn translate_buffer(token: usize, ptr: *const u8, len: usize) -> Vec<&'static mut [u8]> {
    let page_table = PageTable::from_token(token);
    let mut start = ptr as usize;
    let end = start + len;
    let mut v = Vec::new();
    while start < end {
        let start_va = VirtAddr::from(start);
        let mut vpn = start_va.floor();
        let ppn = page_table.translate(vpn).unwrap().ppn();
        vpn.step();
        let mut end_va: VirtAddr = vpn.into();
        end_va = end_va.min(VirtAddr::from(end));
        if end_va.page_offset() == 0 {
            // 此时end_va是下一页的起始地址
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..]);
        } else {
            // 此时end_va和start_va在同一页
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..end_va.page_offset()]);
        }
        start = end_va.into();
    }
    v
}

/// 从内核空间外的某个用户空间获得一个字符串
pub fn translate_str(token: usize, ptr: *const u8) -> String {
    let page_table = PageTable::from_token(token);

    let mut str = String::new();
    let mut va = ptr as usize;
    loop {
        // 因为我们的内核是Identical Mapping，所以翻译得到的物理地址就是内核地址空间的地址，可以直接访问
        let ch: u8 = *(page_table
            .translate_va(VirtAddr::from(va))
            .unwrap()
            .get_mut());
        // println!("sasd");
        if ch == 0 {
            break;
        }
        str.push(ch as char);
        va += 1;
    }
    str
}

/// 把用户地址空间的指针转成内核可操作的引用
pub fn translate_ref_mut<T>(token: usize, ptr: *mut T) -> &'static mut T {
    let page_table = PageTable::from_token(token);
    page_table
        .translate_va((ptr as usize).into())
        .unwrap()
        .get_mut()
}

#[allow(unused)]
pub fn find_pte_test() {
    let mut page_table = PageTable::new();
    let vpn = VirtPageNum(0x123456789);
    let ppn = PhysPageNum(0x987654321);
    let flags = PTEFlags::V | PTEFlags::R | PTEFlags::W;
    page_table.map(vpn, ppn, flags);
    let pte = page_table.find_pte(vpn).unwrap();
    assert_eq!(pte.ppn(), ppn);
    assert_eq!(pte.flags(), flags);
    println!("find_pte_test passed");
}
