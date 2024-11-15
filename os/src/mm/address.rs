use crate::config::{PAGE_SIZE, PAGE_SIZE_BITS};

// PA: Physical Address
pub const PA_WIDTH_SV39: usize = 56;
pub const VA_WIDTH_SV39: usize = 39;
// PPN: Physical Page Number
pub const PPN_WIDTH_SV39: usize = PA_WIDTH_SV39 - PAGE_SIZE_BITS;
pub const VPN_WIDTH_SV39: usize = VA_WIDTH_SV39 - PAGE_SIZE_BITS;

/// Definitions
/// [55:12] 为物理页框号 [11:0] 为页内偏移
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PhysAddr(pub usize);

/// virtual address
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct VirtAddr(pub usize);

/// physical page number
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PhysPageNum(pub usize);

/// virtual page number
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
