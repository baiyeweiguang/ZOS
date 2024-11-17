use core::{cmp::min, iter::Map, mem};

use crate::{
    config::{MEMORY_END, PAGE_SIZE, TRAMPOLINE_ADDRESS, TRAP_CONTEXT_ADDRESS, USER_STACK_SIZE},
    lang_items::StepByOne,
    println,
};

use super::{
    address::{PhysAddr, PhysPageNum, VPNRange, VirtAddr, VirtPageNum},
    frame_allocator::{frame_alloc, FrameTracker},
    page_table::{PTEFlags, PageTable},
};
use alloc::{collections::btree_map::BTreeMap, vec::Vec};
use bitflags::bitflags;
use riscv::addr::page;

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
        page_table.unmap(vpn.0.into());
    }

    pub fn copy_data(&mut self, page_table: &PageTable, data: &[u8]) {
        let len = data.len();
        let mut current_vpn = self.vpn_range.get_start();

        // 一页一页地拷贝
        let mut start = 0;
        loop {
            let src = &data[start..min(len, start + PAGE_SIZE)];

            // let dst_pa: PhysAddr = page_table.translate(current_vpn).unwrap().ppn().into();
            // let dst: &mut [u8] =
            // unsafe { core::slice::from_raw_parts_mut(dst_pa.0 as *mut u8, PAGE_SIZE) };

            // 已经实现了接口将ppn转为&mut [u8]，所以不用上面的那个了
            let dst = page_table
                .translate(current_vpn)
                .unwrap()
                .ppn()
                .get_bytes_array();

            dst.copy_from_slice(src);
            current_vpn.step();

            start += PAGE_SIZE;
            if start >= len {
                break;
            }
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

        memory_set
    }

    // 在地址空间中插入一个新的逻辑段map_area，data为可选的初始化数据
    fn push(&mut self, mut map_area: MapArea, data: Option<&[u8]>) {
        map_area.map(&mut self.page_table);

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
            VirtPageNum(TRAMPOLINE_ADDRESS),
            PhysPageNum(strampoline as usize),
            PTEFlags::R | PTEFlags::X,
        )
    }

    /// Include sections in elf and trampoline and TrapContext and user stack,
    /// also returns user_sp and entry point.
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
        user_stack_bottom += PAGE_SIZE;

        // 用户栈的顶部地址
        let user_stack_top = user_stack_bottom + USER_STACK_SIZE;
        // 创建用户栈的MapArea
        memory_set.push(
            MapArea::new(
                user_stack_bottom.into(),
                user_stack_top.into(),
                MapType::Framed,
                MapPermission::R | MapPermission::W | MapPermission::U,
            ),
            None,
        );

        // 创建TrapContext的MapArea
        memory_set.push(
            MapArea::new(
                TRAP_CONTEXT_ADDRESS.into(),
                TRAMPOLINE_ADDRESS.into(),
                MapType::Framed,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );

        // 返回MemorySet，用户栈的顶部地址，程序入口地址
        (
            memory_set,
            user_stack_top,
            elf.header.pt2.entry_point() as usize,
        )
    }
}
