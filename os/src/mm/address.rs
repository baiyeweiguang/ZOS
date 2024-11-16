use core::fmt::{Debug, Formatter};

use crate::config::{PAGE_SIZE, PAGE_SIZE_BITS};

use super::page_table::PageTableEntry;

// PA: Physical Address
pub const PA_WIDTH_SV39: usize = 56;
pub const VA_WIDTH_SV39: usize = 39;
// PPN: Physical Page Number
pub const PPN_WIDTH_SV39: usize = PA_WIDTH_SV39 - PAGE_SIZE_BITS;
pub const VPN_WIDTH_SV39: usize = VA_WIDTH_SV39 - PAGE_SIZE_BITS;

/// Definitions
/// physical address
/// [63:56] = 0 [55:12] 为物理页框号 [11:0] 为页内偏移
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PhysAddr(pub usize);

/// [64:39] = 0
/// [38:12] 为虚拟页号 [11:0] 为页内偏移
/// 相当于VirtPageNum + PageOffset
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct VirtAddr(pub usize);

/// physical page number
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PhysPageNum(pub usize);

/// virtual page number
// [26:17] 为一级页表索引 [16:9] 为二级页表索引 [8:0] 为三级页表索引
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct VirtPageNum(pub usize);

impl PhysAddr {
    pub fn page_offset(&self) -> usize {
        // PAGE_SIZE - 1 = 0x0000_0000_0000_0fff,取低12位
        self.0 & (PAGE_SIZE - 1)
    }

    pub fn floor(&self) -> PhysPageNum {
        // 因为物理地址=[物理页框号,页内偏移]=物理页框号*PAGE_SIZE+页内偏移
        // 所以物理页框号=物理地址/PAGE_SIZE
        // 换个思路理解，单个页面的大小设置为4KiB，每个虚拟页面和物理页帧都对齐到这个页面大小，
        // 也就是说虚拟/物理地址区间[0.4KiB)为第0个虚拟页面/物理页帧，而[4KiB,8KiB)为第1个，依次类推
        // 所以物理地址/4KiB=物理页帧号
        PhysPageNum(self.0 / PAGE_SIZE)
        // 因为需要保证物理页号与页面大小对齐，才能通过右移转换为物理页号，所以这里只能用 /
    }

    pub fn ceil(&self) -> PhysPageNum {
        PhysPageNum((self.0 + PAGE_SIZE - 1) / PAGE_SIZE)
    }
}

impl PhysPageNum {
    pub fn get_pte_array(&self) -> &'static mut [PageTableEntry] {
        let pa: PhysAddr = self.clone().into();
        unsafe { core::slice::from_raw_parts_mut(pa.0 as *mut PageTableEntry, 512) }
    }

    // As a reference lifetime, &'static ndicates the data
    // pointed to by the reference lives as long as the running program.
    // But it can still be coerced to a shorter lifetime.
    pub fn get_bytes_array(&self) -> &'static mut [u8] {
        let pa: PhysAddr = (*self).into();
        unsafe { core::slice::from_raw_parts_mut(pa.0 as *mut u8, PAGE_SIZE) }
    }

    pub fn get_mut<T>(&self) -> &'static mut T {
        let pa: PhysAddr = self.clone().into();
        unsafe { (pa.0 as *mut T).as_mut().unwrap() }
    }
}

impl VirtPageNum {
    // 获取这个虚拟页表的三级页表的索引
    pub fn indexes(&self) -> [usize; 3] {
        let mut vpn = self.0;
        let mut idx: [usize; 3] = [0; 3];
        for i in (0..3).rev() {
            // 取低9位
            idx[i] = vpn & 0b111111111; // 9 bits
            vpn >>= 9;
        }
        idx
    }
}

impl From<PhysAddr> for PhysPageNum {
    fn from(v: PhysAddr) -> Self {
        assert_eq!(v.page_offset(), 0);
        v.floor()
    }
}

impl From<PhysPageNum> for PhysAddr {
    fn from(v: PhysPageNum) -> Self {
        // Self(v.0 * PAGE_SIZE)
        Self(v.0 << PAGE_SIZE_BITS)
    }
}

impl From<PhysAddr> for usize {
    fn from(v: PhysAddr) -> Self {
        v.0
    }
}

impl From<PhysPageNum> for usize {
    fn from(v: PhysPageNum) -> Self {
        v.0
    }
}

impl From<usize> for PhysAddr {
    fn from(v: usize) -> Self {
        // 1 << PA_WIDTH_SV39 - 1 = 0x00ff_ffff_ffff_ffff, 取低56位
        Self(v & ((1 << PA_WIDTH_SV39) - 1))
    }
}

impl From<usize> for PhysPageNum {
    fn from(v: usize) -> Self {
        Self(v & ((1 << PPN_WIDTH_SV39) - 1))
    }
}

impl From<usize> for VirtAddr {
    fn from(v: usize) -> Self {
        Self(v & ((1 << VA_WIDTH_SV39) - 1))
    }
}
impl From<usize> for VirtPageNum {
    fn from(v: usize) -> Self {
        Self(v & ((1 << VPN_WIDTH_SV39) - 1))
    }
}

impl Debug for PhysPageNum {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("PhysPageNum({:#x})", self.0))
    }
}

impl Debug for VirtPageNum {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("VirtPageNum({:#x})", self.0))
    }
}
