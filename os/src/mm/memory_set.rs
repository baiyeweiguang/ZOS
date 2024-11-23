use core::{arch::asm, cmp::min};

use crate::{
    config::{MEMORY_END, PAGE_SIZE, TRAMPOLINE_ADDRESS, TRAP_CONTEXT_ADDRESS, USER_STACK_SIZE},
    lang_items::StepByOne,
    println,
    sync::UPSafeCell,
};

use super::{
    address::{PhysAddr, PhysPageNum, VPNRange, VirtAddr, VirtPageNum},
    frame_allocator::{frame_alloc, FrameTracker},
    page_table::{self, PTEFlags, PageTable, PageTableEntry},
};
use alloc::{collections::btree_map::BTreeMap, sync::Arc, vec::Vec};
use bitflags::bitflags;
use lazy_static::lazy_static;
use riscv::register::satp;

extern "C" {
    fn strampoline();
    fn stext();
    fn etext();
    fn srodata();
    fn erodata();
    fn sdata();
    fn edata();
    fn sbss_with_stack();
    fn ebss();
    fn ekernel();
}

// 虚拟页面到物理页面的映射类型
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum MapType {
    Identical, // 恒等映射，内核中用
    Framed,    // 有帧映射，用户中用
}

bitflags! {
  pub struct MapPermission: u8 {
      const R = 1 << 1;
      const W = 1 << 2;
      const X = 1 << 3;
      const U = 1 << 4;
  }
}

// 每个进程可能持有多个
pub struct MapArea {
    vpn_range: VPNRange,
    // 用于保存每个虚拟页面与对应的物理页帧的键值对
    // 只有Framed类型的MapArea才会用到这个字段
    data_frames: BTreeMap<VirtPageNum, FrameTracker>,
    map_type: MapType,
    map_perm: MapPermission,
}

impl MapArea {
    pub fn new(
        start_va: VirtAddr,
        end_va: VirtAddr,
        map_type: MapType,
        map_perm: MapPermission,
    ) -> Self {
        let start_vpn: VirtPageNum = start_va.floor();
        let end_vpn: VirtPageNum = end_va.ceil();
        Self {
            vpn_range: VPNRange::new(start_vpn, end_vpn),
            data_frames: BTreeMap::new(),
            map_type,
            map_perm,
        }
    }

    pub fn map(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.map_one(page_table, vpn);
        }
    }

    pub fn map_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        let ppn: PhysPageNum = match self.map_type {
            MapType::Identical => PhysPageNum(vpn.0),
            MapType::Framed => {
                let frame = frame_alloc().unwrap();
                let ppn = frame.ppn;
                self.data_frames.insert(vpn, frame);
                ppn
            }
        };

        let flags = PTEFlags::from_bits(self.map_perm.bits).unwrap();
        page_table.map(vpn, ppn, flags);
    }

    #[allow(unused)]
    /// unmap所有
    pub fn unmap(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.unmap_one(page_table, vpn);
        }
    }

    #[allow(unused)]
    pub fn unmap_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        // let ppn = match self.map_type {
        //   MapType::Identical => PhysPageNum(vpn.0),
        //   MapType::Framed => {
        //     let frame = self.data_frames.remove(&vpn).unwrap();
        //     frame.ppn
        //   }
        // };
        // page_table.unmap(ppn);

        // 为什么不是上面的逻辑？
        // 哦，下面只是上面的简化版，没事了
        if self.map_type == MapType::Framed {
            self.data_frames.remove(&vpn);
        }
        page_table.unmap(vpn);
    }

    #[allow(unused)]
    pub fn shrink_to(&mut self, page_table: &mut PageTable, new_end: VirtPageNum) {
        for vpn in VPNRange::new(new_end, self.vpn_range.get_end()) {
            self.unmap_one(page_table, vpn);
        }
        self.vpn_range = VPNRange::new(self.vpn_range.get_start(), new_end);
    }

    #[allow(unused)]
    pub fn append_to(&mut self, page_table: &mut PageTable, new_end: VirtPageNum) {
        for vpn in VPNRange::new(self.vpn_range.get_end(), new_end) {
            self.map_one(page_table, vpn);
        }
        self.vpn_range = VPNRange::new(self.vpn_range.get_start(), new_end);
    }

    /// data: start-aligned but maybe with shorter length
    /// assume that all frames were cleared before
    pub fn copy_data(&mut self, page_table: &PageTable, data: &[u8]) {
        assert_eq!(self.map_type, MapType::Framed);
        let mut start: usize = 0;
        let mut current_vpn = self.vpn_range.get_start();
        let len = data.len();
        loop {
            let src = &data[start..len.min(start + PAGE_SIZE)];
            let dst = &mut page_table
                .translate(current_vpn)
                .unwrap()
                .ppn()
                .get_bytes_array()[..src.len()];
            dst.copy_from_slice(src);
            start += PAGE_SIZE;
            if start >= len {
                break;
            }
            current_vpn.step();
        }
    }

    /// 从另一个MapArea构造新的MapArea，注意这个不会复制数据
    pub fn from_another(another: &MapArea) -> Self {
        Self {
            vpn_range: VPNRange::new(another.vpn_range.get_start(), another.vpn_range.get_end()),
            data_frames: BTreeMap::new(),
            map_type: another.map_type,
            map_perm: another.map_perm,
        }
    }
}

// 每个进程持有一个
pub struct MemorySet {
    page_table: PageTable,
    areas: Vec<MapArea>,
}

impl MemorySet {
    pub fn new_bare() -> Self {
        Self {
            page_table: PageTable::new(),
            areas: Vec::new(),
        }
    }

    pub fn token(&self) -> usize {
        self.page_table.token()
    }

    // 启用地址空间
    pub fn activate(&self) {
        let satp = self.token();
        unsafe {
            satp::write(satp);
            // sfence.vma指令用于刷新TLB
            asm!("sfence.vma");
        }
    }

    pub fn from_existed_user(user_space: &MemorySet) -> Self {
        let mut new_memory_set = Self::new_bare();
        new_memory_set.map_trampoline();

        // copy data sections/trap_context/user_stack
        for area in user_space.areas.iter() {
            // 这里的new_area还没有实际映射到物理页帧
            let new_area = MapArea::from_another(&area);

            // push的时候会进行映射
            new_memory_set.push(new_area, None);

            // 因为两个area的vpn_range是相同的，
            // 所以在虚拟地址空间上看，两者是一样的
            // 但是内部映射到的物理页帧是不一样的
            for vpn in area.vpn_range {
                let src_ppn = user_space.translate(vpn).unwrap().ppn();
                let dst_ppn = new_memory_set.translate(vpn).unwrap().ppn();

                // 因为已经映射到了物理页帧，所以这里可以直接copy
                dst_ppn
                    .get_bytes_array()
                    .copy_from_slice(src_ppn.get_bytes_array());
            }
        }

        new_memory_set
    }

    /// 释放用户空间的内存
    pub fn recycle_data_pages(&mut self) {
        // 逻辑上的释放，其实并没有擦除物理内存
        // clear后，这个地址空间就无效了，因为page_table没了
        // 但是物理页帧还在，他们会由父进程最后回收
        self.areas.clear();
    }

    // 创建内核的地址空间
    pub fn new_kernel() -> Self {
        let mut memory_set = Self::new_bare();
        memory_set.map_trampoline();

        println!(".text [{:#x}, {:#x})", stext as usize, etext as usize);
        println!(".rodata [{:#x}, {:#x})", srodata as usize, erodata as usize);
        println!(".data [{:#x}, {:#x})", sdata as usize, edata as usize);
        println!(
            ".bss [{:#x}, {:#x})",
            sbss_with_stack as usize, ebss as usize
        );
        println!("mapping .text section");
        memory_set.push(
            MapArea::new(
                VirtAddr::from(stext as usize),
                VirtAddr::from(etext as usize),
                MapType::Identical,
                MapPermission::R | MapPermission::X,
            ),
            None,
            // Some(unsafe { core::slice::from_raw_parts_mut(stext as usize as *mut u8, etext as usize - stext as usize) }),
        );
        println!("mapping .rodata section");
        memory_set.push(
            MapArea::new(
                VirtAddr::from(srodata as usize),
                VirtAddr::from(erodata as usize),
                MapType::Identical,
                MapPermission::R,
            ),
            None,
        );
        println!("mapping .data section");
        memory_set.push(
            MapArea::new(
                VirtAddr::from(sdata as usize),
                VirtAddr::from(edata as usize),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        println!("mapping .bss section");
        memory_set.push(
            MapArea::new(
                (sbss_with_stack as usize).into(),
                (ebss as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        println!("mapping left physical memory");
        memory_set.push(
            MapArea::new(
                (ekernel as usize).into(),
                MEMORY_END.into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        println!("kernel memory space setup success!");
        memory_set
    }

    // 在地址空间中插入一个新的逻辑段map_area，data为可选的初始化数据
    fn push(&mut self, mut map_area: MapArea, data: Option<&[u8]>) {
        map_area.map(&mut self.page_table);
        // println!("MapType: {:?}", map_area.map_type);
        if let Some(data) = data {
            map_area.copy_data(&self.page_table, data);
        }
        self.areas.push(map_area);
    }

    pub fn insert_framed_area(
        &mut self,
        start_va: VirtAddr,
        end_va: VirtAddr,
        permission: MapPermission,
    ) {
        self.push(
            MapArea::new(start_va, end_va, MapType::Framed, permission),
            None,
        );
    }

    fn map_trampoline(&mut self) {
        self.page_table.map(
            VirtAddr::from(TRAMPOLINE_ADDRESS).into(),
            PhysAddr::from(strampoline as usize).into(),
            PTEFlags::R | PTEFlags::X,
        );
    }

    #[allow(unused)]
    pub fn shrink_to(&mut self, start: VirtAddr, new_end: VirtAddr) -> bool {
        if let Some(area) = self
            .areas
            .iter_mut()
            .find(|area| area.vpn_range.get_start() == start.floor())
        {
            area.shrink_to(&mut self.page_table, new_end.floor());
            true
        } else {
            false
        }
    }

    #[allow(unused)]
    pub fn append_to(&mut self, start: VirtAddr, new_end: VirtAddr) -> bool {
        if let Some(area) = self
            .areas
            .iter_mut()
            .find(|area| area.vpn_range.get_start() == start.floor())
        {
            true
        } else {
            false
        }
    }

    /// Include sections in elf and trampoline and TrapContext and user stack,
    /// also returns user_sp and entry point.
    /// 返回(memory_set, user_sp, entry_point)
    // 从elf文件中加载用户程序，创建其地址空间
    pub fn from_elf(elf_data: &[u8]) -> (Self, usize, usize) {
        let mut memory_set = Self::new_bare();
        // map trampoline
        memory_set.map_trampoline();
        // 用xmas-elf库解析elf文件
        let elf = xmas_elf::ElfFile::new(elf_data).unwrap();
        let elf_header = elf.header;

        // 根据magic number判断elf文件是否合法
        let magic = elf_header.pt1.magic;
        assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "invalid elf!");

        // ph: program header
        // ​存放的是系统加载可执行程序所需要的所有信息，是程序装载必须的一部分。
        // Program header 是由一个或多个相同结构的程序段(Segment)组成的。

        // 每个程序段(Segment)用于描述一段硬盘数据和内存数据.
        // pt1为elf_header的基础元信息，pt2是程序加载和链接的描述
        let ph_count = elf_header.pt2.ph_count();
        let mut max_end_vpn = VirtPageNum(0);

        // 为每个程序段创建一个MapArea
        for i in 0..ph_count {
            let ph = elf.program_header(i).unwrap();
            if ph.get_type().unwrap() == xmas_elf::program::Type::Load {
                // 获取地址范围
                let start_va: VirtAddr = (ph.virtual_addr() as usize).into();
                let end_va: VirtAddr = ((ph.virtual_addr() + ph.mem_size()) as usize).into();
                // 设置权限标志位
                let mut map_perm = MapPermission::U;
                let ph_flags = ph.flags();
                if ph_flags.is_read() {
                    map_perm |= MapPermission::R;
                }
                if ph_flags.is_write() {
                    map_perm |= MapPermission::W;
                }
                if ph_flags.is_execute() {
                    map_perm |= MapPermission::X;
                }
                // 创建MapArea
                // println!("App {}， start_va: {:#x}, end_va: {:#x}", i, start_va.0, end_va.0);
                let map_area = MapArea::new(start_va, end_va, MapType::Framed, map_perm);
                max_end_vpn = map_area.vpn_range.get_end();
                memory_set.push(
                    map_area,
                    Some(&elf.input[ph.offset() as usize..(ph.offset() + ph.file_size()) as usize]),
                );
            }
        }

        // 计算用户栈的起始地址
        // map user stack with U flags
        let max_end_va: VirtAddr = max_end_vpn.into();
        let mut user_stack_bottom: usize = max_end_va.into();

        // 栈的前一页留给guard page
        // 所以内存分布为[.text, .rodata, .data, .bss, guard page, user stack, (very big space), trap context, trampoline]
        // 在这个版本中，似乎每个程序是没有堆的，所有程序共用内核.bss段的空间作为堆
        user_stack_bottom += PAGE_SIZE;

        // 返回MemorySet，用户栈基地址，程序入口地址
        (
            memory_set,
            user_stack_bottom,
            elf.header.pt2.entry_point() as usize,
        )
    }

    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.page_table.translate(vpn)
    }

    ///Remove `MapArea` that starts with `start_vpn`
    pub fn remove_area_with_start_vpn(&mut self, start_vpn: VirtPageNum) {
        if let Some((idx, area)) = self
            .areas
            .iter_mut()
            .enumerate()
            .find(|(_, area)| area.vpn_range.get_start() == start_vpn)
        {
            area.unmap(&mut self.page_table);
            self.areas.remove(idx);
        }
    }
}

// 全局的内核地址空间
lazy_static! {
    pub static ref KERNEL_SPACE: Arc<UPSafeCell<MemorySet>> =
        Arc::new(UPSafeCell::new(MemorySet::new_kernel()));
}

pub fn kernel_token() -> usize {
    KERNEL_SPACE.exclusive_access().token()
}

// 一个简单的检查程序
#[allow(unused)]
pub fn remap_test() {
    let memory_set = KERNEL_SPACE.exclusive_access();

    // RX
    let mid_text: VirtAddr = (stext as usize + (etext as usize - stext as usize) / 2).into();
    // R
    let mid_data: VirtAddr = (sdata as usize + (edata as usize - sdata as usize) / 2).into();
    // RW
    let mid_bss: VirtAddr =
        (sbss_with_stack as usize + (ebss as usize - sbss_with_stack as usize) / 2).into();

    assert_eq!(
        memory_set
            .page_table
            .translate(mid_text.floor())
            .unwrap()
            .writable(),
        false
    );

    assert_eq!(
        memory_set
            .page_table
            .translate(mid_data.floor())
            .unwrap()
            .writable(),
        false
    );

    assert_eq!(
        memory_set
            .page_table
            .translate(mid_bss.floor())
            .unwrap()
            .executable(),
        false
    );

    println!("remap test passed!");
}
