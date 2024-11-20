//! Loading user applications into memory

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use crate::println;

/// Get the total number of applications.
pub fn get_num_app() -> usize {
    extern "C" {
        fn _num_app();
    }
    unsafe { (_num_app as usize as *const usize).read_volatile() }
}

/// get applications data, return elf data
// 这里的地址都是虚拟地址
pub fn get_app_data(app_id: usize) -> &'static [u8] {
    extern "C" {
        fn _num_app();
    }
    let num_app_ptr = _num_app as usize as *const usize;
    let num_app = get_num_app();
    let app_start = unsafe { core::slice::from_raw_parts(num_app_ptr.add(1), num_app + 1) };
    assert!(app_id < num_app);
    unsafe {
        core::slice::from_raw_parts(
            app_start[app_id] as *const u8,
            app_start[app_id + 1] - app_start[app_id],
        )
    }
}

lazy_static! {
    static ref APP_NAMES_MAP: BTreeMap<&'static str, usize> = {
        let num_app = get_num_app();
        extern "C" {
            fn _app_names();
        }
        let mut start = _app_names as usize as *const u8;
        let mut map = BTreeMap::new();
        for i in 0..num_app {
            let mut end = start;
            unsafe {
                while end.read_volatile() != '\0' as u8 {
                    end = end.add(1);
                }
            }
            let slice =
                unsafe { core::slice::from_raw_parts(start, end as usize - start as usize) };
            let str = core::str::from_utf8(slice).unwrap();
            map.insert(str, i);
            start = unsafe { end.add(1) };
        }
        map
    };
}

pub fn get_app_data_by_name(name: &str) -> Option<&'static [u8]> {
    APP_NAMES_MAP.get(name).map(|&id| get_app_data(id))
}

pub fn list_apps() {
    println!("/**** APPS ****");
    for (name, id) in APP_NAMES_MAP.iter() {
        println!("Name: {},id: {}", name, id);
    }
    println!("**************/");
}