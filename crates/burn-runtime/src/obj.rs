use crate::gc;
use crate::meta::{TID_ARR_STR, TID_STR};
use std::alloc::{alloc_zeroed, dealloc, realloc, Layout};
use std::sync::{Arc, Condvar, Mutex};

pub const K_STR: u8 = 1;
pub const K_ARRAY: u8 = 2;
pub const K_STRUCT: u8 = 3;
pub const K_BOX: u8 = 4;
pub const K_MAP: u8 = 5;
pub const K_FUTURE: u8 = 6;

pub const F_STATIC: u8 = 1;
pub const F_ASCII: u8 = 2;

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
    pub size: u64,
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
    let p = gc::alloc(K_STR, TID_STR, size);
    unsafe {
        set_word(p, STR_LEN, s.len() as u64);
        std::ptr::copy_nonoverlapping(s.as_ptr(), (p as usize + STR_BYTES) as *mut u8, s.len());
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
            std::alloc::handle_alloc_error(layout);
        }
        let h = hdr(p);
        h.kind = K_STR;
        h.flags = F_STATIC | if s.is_ascii() { F_ASCII } else { 0 };
        h.tid = TID_STR;
        h.size = size as u64;
        set_word(p, STR_LEN, s.len() as u64);
        std::ptr::copy_nonoverlapping(s.as_ptr(), (p as usize + STR_BYTES) as *mut u8, s.len());
        p
    }
}

#[inline]
pub fn str_bytes<'a>(p: u64) -> &'a [u8] {
    unsafe { std::slice::from_raw_parts((p as usize + STR_BYTES) as *const u8, word(p, STR_LEN) as usize) }
}

#[inline]
pub fn str_ref<'a>(p: u64) -> &'a str {
    unsafe { std::str::from_utf8_unchecked(str_bytes(p)) }
}

#[inline]
pub fn str_is_ascii(p: u64) -> bool {
    unsafe { hdr(p).flags & F_ASCII != 0 }
}

pub fn array_new(tid: u32, len: usize) -> u64 {
    let p = gc::alloc(K_ARRAY, tid, HDR + 24);
    let cap = len.max(4);
    unsafe {
        let data = alloc_zeroed(Layout::array::<u64>(cap).unwrap()) as u64;
        set_word(p, ARR_LEN, len as u64);
        set_word(p, ARR_CAP, cap as u64);
        set_word(p, ARR_DATA, data);
    }
    gc::account(cap * 8);
    p
}

pub fn array_from(tid: u32, items: &[u64]) -> u64 {
    let p = array_new(tid, items.len());
    unsafe {
        std::ptr::copy_nonoverlapping(items.as_ptr(), array_data(p), items.len());
    }
    p
}

pub fn array_of_strings(items: &[String]) -> u64 {
    let arr = array_new(TID_ARR_STR, 0);
    let _g = gc::root(arr);
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
    unsafe { std::slice::from_raw_parts(array_data(p), array_len(p)) }
}

#[inline]
pub fn array_slice_mut<'a>(p: u64) -> &'a mut [u64] {
    unsafe { std::slice::from_raw_parts_mut(array_data(p), array_len(p)) }
}

pub fn array_reserve(p: u64, need: usize) {
    unsafe {
        let cap = word(p, ARR_CAP) as usize;
        if need <= cap {
            return;
        }
        let new_cap = need.max(cap * 2);
        let old = Layout::array::<u64>(cap).unwrap();
        let data = realloc(array_data(p) as *mut u8, old, new_cap * 8) as u64;
        if data == 0 {
            std::alloc::handle_alloc_error(Layout::array::<u64>(new_cap).unwrap());
        }
        std::ptr::write_bytes((data as usize + cap * 8) as *mut u8, 0, (new_cap - cap) * 8);
        set_word(p, ARR_DATA, data);
        set_word(p, ARR_CAP, new_cap as u64);
        gc::account((new_cap - cap) * 8);
    }
}

pub fn array_push(p: u64, v: u64) {
    let len = array_len(p);
    array_reserve(p, len + 1);
    unsafe {
        *array_data(p).add(len) = v;
        set_word(p, ARR_LEN, (len + 1) as u64);
    }
}

pub fn array_set_len(p: u64, len: usize) {
    array_reserve(p, len);
    unsafe { set_word(p, ARR_LEN, len as u64) }
}

pub unsafe fn array_free_data(p: u64) {
    let cap = word(p, ARR_CAP) as usize;
    let data = word(p, ARR_DATA);
    if data != 0 {
        dealloc(data as usize as *mut u8, Layout::array::<u64>(cap).unwrap());
    }
}

pub fn struct_new(tid: u32, n: usize) -> u64 {
    gc::alloc(K_STRUCT, tid, HDR + 8 * n.max(1))
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
    let p = gc::alloc(K_BOX, tid, HDR + 8);
    unsafe { set_word(p, BOX_VAL, v) }
    p
}

#[inline(always)]
pub fn box_val(p: u64) -> u64 {
    unsafe { word(p, BOX_VAL) }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum MapKey {
    I(u64),
    S(Box<[u8]>),
}

#[derive(Default)]
pub struct MapData {
    pub keys: Vec<u64>,
    pub vals: Vec<u64>,
    pub index: std::collections::HashMap<MapKey, usize, crate::fx::FxBuild>,
}

pub fn map_new(tid: u32) -> u64 {
    let p = gc::alloc(K_MAP, tid, HDR + 8);
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

pub struct FutureState {
    pub value: Mutex<Option<u64>>,
    pub cv: Condvar,
}

pub fn future_new(tid: u32) -> u64 {
    let p = gc::alloc(K_FUTURE, tid, HDR + 8);
    let st = Arc::new(FutureState { value: Mutex::new(None), cv: Condvar::new() });
    unsafe { set_word(p, HDR, Arc::into_raw(st) as u64) }
    p
}

pub fn future_state(p: u64) -> Arc<FutureState> {
    unsafe {
        let raw = word(p, HDR) as usize as *const FutureState;
        Arc::increment_strong_count(raw);
        Arc::from_raw(raw)
    }
}

pub unsafe fn future_free(p: u64) {
    let raw = word(p, HDR) as usize as *const FutureState;
    if !raw.is_null() {
        drop(Arc::from_raw(raw));
    }
}
