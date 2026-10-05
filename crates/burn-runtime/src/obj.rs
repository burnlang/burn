use crate::meta::{TID_ARR_STR, TID_STR};
#[allow(unused_imports)]
use crate::prelude::*;
use crate::rc;
use alloc::alloc::{alloc_zeroed, dealloc, realloc, Layout};
#[cfg(not(burn_core))]
use std::sync::{Arc, Condvar, Mutex};

pub const K_STR: u8 = 1;
pub const K_ARRAY: u8 = 2;
pub const K_STRUCT: u8 = 3;
pub const K_BOX: u8 = 4;
pub const K_MAP: u8 = 5;
pub const K_FUTURE: u8 = 6;
pub const K_DEAD: u8 = 7;

pub const F_STATIC: u8 = 1;
pub const F_ASCII: u8 = 2;
pub const F_TRACK: u8 = 4;
pub const F_BUFFERED: u8 = 8;
pub const F_ZOMBIE: u8 = 16;

pub const HDR: usize = 16;
pub const ARR_LEN: usize = 16;
pub const ARR_CAP: usize = 24;
pub const ARR_DATA: usize = 32;
pub const STR_LEN: usize = 16;
pub const STR_BYTES: usize = 24;
pub const BOX_VAL: usize = 16;

#[repr(C)]
pub struct Header {
    pub kind: u8,
    pub mark: u8,
    pub flags: u8,
    pub pad: u8,
    pub tid: u32,
    pub size: u32,
    pub rc: u32,
}

#[inline(always)]
pub unsafe fn hdr<'a>(p: u64) -> &'a mut Header {
    &mut *(p as usize as *mut Header)
}

#[inline(always)]
pub unsafe fn word(p: u64, off: usize) -> u64 {
    *((p as usize + off) as *const u64)
}

#[inline(always)]
pub unsafe fn set_word(p: u64, off: usize, v: u64) {
    *((p as usize + off) as *mut u64) = v
}

#[inline(always)]
pub fn tid_of(p: u64) -> u32 {
    unsafe { hdr(p).tid }
}

#[inline(always)]
pub fn kind_of(p: u64) -> u8 {
    unsafe { hdr(p).kind }
}

pub fn str_new(s: &[u8]) -> u64 {
    let size = HDR + 8 + s.len() + 1;
    let p = rc::alloc(K_STR, TID_STR, size);
    unsafe {
        set_word(p, STR_LEN, s.len() as u64);
        core::ptr::copy_nonoverlapping(s.as_ptr(), (p as usize + STR_BYTES) as *mut u8, s.len());
        if s.is_ascii() {
            hdr(p).flags |= F_ASCII;
        }
    }
    p
}

pub fn string(s: &str) -> u64 {
    str_new(s.as_bytes())
}

pub fn str_static(s: &[u8]) -> u64 {
    let size = HDR + 8 + s.len() + 1;
    unsafe {
        let layout = Layout::from_size_align(size, 16).unwrap();
        let p = alloc_zeroed(layout) as u64;
        if p == 0 {
            alloc::alloc::handle_alloc_error(layout);
        }
        let h = hdr(p);
        h.kind = K_STR;
        h.flags = F_STATIC | if s.is_ascii() { F_ASCII } else { 0 };
        h.tid = TID_STR;
        h.size = size as u32;
        set_word(p, STR_LEN, s.len() as u64);
        core::ptr::copy_nonoverlapping(s.as_ptr(), (p as usize + STR_BYTES) as *mut u8, s.len());
        p
    }
}

#[inline]
pub fn str_bytes<'a>(p: u64) -> &'a [u8] {
    unsafe { core::slice::from_raw_parts((p as usize + STR_BYTES) as *const u8, word(p, STR_LEN) as usize) }
}

#[inline]
pub fn str_ref<'a>(p: u64) -> &'a str {
    unsafe { core::str::from_utf8_unchecked(str_bytes(p)) }
}

#[inline]
pub fn str_is_ascii(p: u64) -> bool {
    unsafe { hdr(p).flags & F_ASCII != 0 }
}

fn elem_num(tid: u32) -> u8 {
    match crate::meta::desc(tid) {
        crate::meta::Desc::Array(e) => match crate::meta::desc(*e) {
            crate::meta::Desc::Num(n) if n.packed() => *n as u8,
            _ => 0,
        },
        _ => 0,
    }
}

#[inline(always)]
pub fn array_elem(p: u64) -> u8 {
    unsafe { hdr(p).pad }
}

#[inline(always)]
pub fn elem_width(code: u8) -> usize {
    match code {
        0 => 8,
        1 | 2 => 1,
        3 | 4 => 2,
        _ => 4,
    }
}

fn data_layout(cap: usize, code: u8) -> Layout {
    Layout::from_size_align((cap * elem_width(code)).max(8), 8).unwrap()
}

pub fn array_new(tid: u32, len: usize) -> u64 {
    let p = rc::alloc(K_ARRAY, tid, HDR + 24);
    let code = elem_num(tid);
    let cap = len.max(4);
    unsafe {
        hdr(p).pad = code;
        let data = alloc_zeroed(data_layout(cap, code)) as u64;
        set_word(p, ARR_LEN, len as u64);
        set_word(p, ARR_CAP, cap as u64);
        set_word(p, ARR_DATA, data);
    }
    rc::account(cap * elem_width(code));
    p
}

pub fn array_from(tid: u32, items: &[u64]) -> u64 {
    let p = array_new(tid, items.len());
    if array_elem(p) == 0 {
        unsafe {
            core::ptr::copy_nonoverlapping(items.as_ptr(), array_data(p), items.len());
        }
    } else {
        for (i, v) in items.iter().enumerate() {
            array_put(p, i, *v);
        }
    }
    p
}

pub fn array_of_strings(items: &[String]) -> u64 {
    let arr = array_new(TID_ARR_STR, 0);
    for s in items {
        let v = string(s);
        array_push(arr, v);
    }
    arr
}

#[inline(always)]
pub fn array_len(p: u64) -> usize {
    unsafe { word(p, ARR_LEN) as usize }
}

#[inline(always)]
pub fn array_data(p: u64) -> *mut u64 {
    unsafe { word(p, ARR_DATA) as usize as *mut u64 }
}

#[inline]
pub fn array_slice<'a>(p: u64) -> &'a [u64] {
    debug_assert!(array_elem(p) == 0);
    unsafe { core::slice::from_raw_parts(array_data(p), array_len(p)) }
}

#[inline]
pub fn array_slice_mut<'a>(p: u64) -> &'a mut [u64] {
    debug_assert!(array_elem(p) == 0);
    unsafe { core::slice::from_raw_parts_mut(array_data(p), array_len(p)) }
}

#[inline]
pub fn array_at(p: u64, i: usize) -> u64 {
    let d = array_data(p) as usize;
    unsafe {
        match array_elem(p) {
            0 => *(d as *const u64).add(i),
            1 => *(d as *const i8).add(i) as i64 as u64,
            2 => *(d as *const u8).add(i) as u64,
            3 => *(d as *const i16).add(i) as i64 as u64,
            4 => *(d as *const u16).add(i) as u64,
            5 => *(d as *const i32).add(i) as i64 as u64,
            6 => *(d as *const u32).add(i) as u64,
            _ => (*(d as *const f32).add(i) as f64).to_bits(),
        }
    }
}

#[inline]
pub fn array_put(p: u64, i: usize, v: u64) {
    let d = array_data(p) as usize;
    unsafe {
        match array_elem(p) {
            0 => *(d as *mut u64).add(i) = v,
            1 | 2 => *(d as *mut u8).add(i) = v as u8,
            3 | 4 => *(d as *mut u16).add(i) = v as u16,
            5 | 6 => *(d as *mut u32).add(i) = v as u32,
            _ => *(d as *mut f32).add(i) = f64::from_bits(v) as f32,
        }
    }
}

pub fn array_values(p: u64) -> Vec<u64> {
    if array_elem(p) == 0 {
        return array_slice(p).to_vec();
    }
    (0..array_len(p)).map(|i| array_at(p, i)).collect()
}

pub fn array_move(p: u64, from: usize, to: usize, n: usize) {
    let w = elem_width(array_elem(p));
    unsafe {
        let d = array_data(p) as usize as *mut u8;
        core::ptr::copy(d.add(from * w), d.add(to * w), n * w);
    }
}

pub fn array_reserve(p: u64, need: usize) {
    unsafe {
        let cap = word(p, ARR_CAP) as usize;
        if need <= cap {
            return;
        }
        let code = array_elem(p);
        let w = elem_width(code);
        let new_cap = need.max(cap * 2);
        let old = data_layout(cap, code);
        let new_size = data_layout(new_cap, code).size();
        let data = realloc(array_data(p) as *mut u8, old, new_size) as u64;
        if data == 0 {
            alloc::alloc::handle_alloc_error(data_layout(new_cap, code));
        }
        core::ptr::write_bytes((data as usize + old.size()) as *mut u8, 0, new_size - old.size());
        set_word(p, ARR_DATA, data);
        set_word(p, ARR_CAP, new_cap as u64);
        rc::account((new_cap - cap) * w);
    }
}

pub fn array_push(p: u64, v: u64) {
    let len = array_len(p);
    array_reserve(p, len + 1);
    array_put(p, len, v);
    unsafe { set_word(p, ARR_LEN, (len + 1) as u64) }
}

pub fn array_set_len(p: u64, len: usize) {
    array_reserve(p, len);
    unsafe { set_word(p, ARR_LEN, len as u64) }
}

pub unsafe fn array_free_data(p: u64) {
    let cap = word(p, ARR_CAP) as usize;
    let data = word(p, ARR_DATA);
    if data != 0 {
        dealloc(data as usize as *mut u8, data_layout(cap, array_elem(p)));
    }
}

pub fn struct_new(tid: u32, n: usize) -> u64 {
    rc::alloc(K_STRUCT, tid, HDR + 8 * n.max(1))
}

pub fn struct_len(p: u64) -> usize {
    unsafe { (hdr(p).size as usize - HDR) / 8 }
}

#[inline(always)]
pub fn field(p: u64, i: usize) -> u64 {
    unsafe { word(p, HDR + 8 * i) }
}

#[inline(always)]
pub fn set_field(p: u64, i: usize, v: u64) {
    unsafe { set_word(p, HDR + 8 * i, v) }
}

pub fn box_raw(tid: u32, v: u64) -> u64 {
    let p = rc::alloc(K_BOX, tid, HDR + 8);
    unsafe { set_word(p, BOX_VAL, v) }
    p
}

#[inline(always)]
pub fn box_val(p: u64) -> u64 {
    unsafe { word(p, BOX_VAL) }
}

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MapKey {
    I(u64),
    S(Box<[u8]>),
}

#[cfg(not(burn_core))]
pub type MapIndex = std::collections::HashMap<MapKey, usize, crate::fx::FxBuild>;
#[cfg(burn_core)]
pub type MapIndex = alloc::collections::BTreeMap<MapKey, usize>;

#[derive(Default)]
pub struct MapData {
    pub keys: Vec<u64>,
    pub vals: Vec<u64>,
    pub index: MapIndex,
}

pub fn map_new(tid: u32) -> u64 {
    let p = rc::alloc(K_MAP, tid, HDR + 8);
    let data = Box::into_raw(Box::new(MapData::default()));
    unsafe { set_word(p, HDR, data as u64) }
    p
}

#[inline]
pub fn map_data<'a>(p: u64) -> &'a mut MapData {
    unsafe { &mut *(word(p, HDR) as usize as *mut MapData) }
}

pub unsafe fn map_free(p: u64) {
    let d = word(p, HDR) as usize as *mut MapData;
    if !d.is_null() {
        drop(Box::from_raw(d));
    }
}

#[cfg(not(burn_core))]
#[cfg(not(burn_core))]
pub struct FutureState {
    pub value: Mutex<Option<u64>>,
    pub cv: Condvar,
}

#[cfg(not(burn_core))]
pub fn future_new(tid: u32) -> u64 {
    let p = rc::alloc(K_FUTURE, tid, HDR + 8);
    let st = Arc::new(FutureState {
        value: Mutex::new(None),
        cv: Condvar::new(),
    });
    unsafe { set_word(p, HDR, Arc::into_raw(st) as u64) }
    p
}

#[cfg(not(burn_core))]
pub fn future_state(p: u64) -> Arc<FutureState> {
    unsafe {
        let raw = word(p, HDR) as usize as *const FutureState;
        Arc::increment_strong_count(raw);
        Arc::from_raw(raw)
    }
}

#[cfg(not(burn_core))]
pub unsafe fn future_free(p: u64) {
    let raw = word(p, HDR) as usize as *const FutureState;
    if !raw.is_null() {
        drop(Arc::from_raw(raw));
    }
}
