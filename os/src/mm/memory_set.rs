use core::{cmp::min, iter::Map};

use crate::{config::PAGE_SIZE, lang_items::StepByOne};

use super::{
    address::{PhysAddr, PhysPageNum, VPNRange, VirtAddr, VirtPageNum},
    frame_allocator::{frame_alloc, FrameTracker},
    page_table::{PTEFlags, PageTable},
};
use alloc::{collections::btree_map::BTreeMap, vec::Vec};
use bitflags::bitflags;

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
    pub fn unmap_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        // let ppn = match self.map_type {
        //   MapType::Identical => PhysPageNum(vpn.0),
        //   MapType::Framed => {
        //     let frame = self.data_frames.remove(&vpn).unwrap();
        //     frame.ppn
        //   }
        // };
        // page_table.unmap(vpn);
        // frame_dealloc(ppn);

        // 为什么不是上面的逻辑？
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

    // 在地址空间中插入一个新的逻辑段map_area，data为可选的初始化数据
    fn push(&mut self, mut map_area: MapArea, data: Option<&[u8]>) {
        map_area.map(&mut self.page_table);
        if let Some(data) = data {}
    }
}
