use crate::fmt;
use crate::gc;
use crate::io::{self, rt_error};
use crate::meta::{self, desc, Desc, TID_ARR_INT, TID_INT, TID_STR};
use crate::obj::*;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

#[inline]
fn b(v: bool) -> u64 {
    v as u64
}

#[inline]
fn f(v: u64) -> f64 {
    f64::from_bits(v)
}

#[inline]
fn fv(x: f64) -> u64 {
    x.to_bits()
}

fn char_count(s: u64) -> usize {
    if str_is_ascii(s) {
        str_bytes(s).len()
    } else {
        str_ref(s).chars().count()
    }
}

fn byte_offset(s: u64, ci: usize) -> usize {
    if str_is_ascii(s) {
        return ci.min(str_bytes(s).len());
    }
    let st = str_ref(s);
    st.char_indices().nth(ci).map(|(i, _)| i).unwrap_or(st.len())
}

pub fn str_concat(a: u64, c: u64) -> u64 {
    let (x, y) = (str_bytes(a), str_bytes(c));
    if x.is_empty() {
        return c;
    }
    if y.is_empty() {
        return a;
    }
    let mut v = Vec::with_capacity(x.len() + y.len());
    v.extend_from_slice(x);
    v.extend_from_slice(y);
    str_new(&v)
}

pub fn str_eq(a: u64, c: u64) -> u64 {
    b(a == c || str_bytes(a) == str_bytes(c))
}

pub fn str_cmp(a: u64, c: u64) -> u64 {
    match str_bytes(a).cmp(str_bytes(c)) {
        std::cmp::Ordering::Less => (-1i64) as u64,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

pub fn str_len(s: u64) -> u64 {
    char_count(s) as u64
}

pub fn str_index(s: u64, i: u64, loc: u64) -> u64 {
    let n = char_count(s);
    let idx = i as i64;
    if idx < 0 || idx as usize >= n {
        rt_error(&format!("string index {} out of bounds (length {})", idx, n), loc);
    }
    if str_is_ascii(s) {
        let bt = str_bytes(s)[idx as usize];
        return str_new(&[bt]);
    }
    let c = str_ref(s).chars().nth(idx as usize).unwrap();
    let mut buf = [0u8; 4];
    str_new(c.encode_utf8(&mut buf).as_bytes())
}

pub fn str_sub(s: u64, a: u64, e: u64) -> u64 {
    let n = char_count(s) as i64;
    let mut start = (a as i64).clamp(0, n);
    let end = (e as i64).clamp(0, n);
    if start > end {
        start = end;
    }
    let (bs, be) = (byte_offset(s, start as usize), byte_offset(s, end as usize));
    str_new(&str_bytes(s)[bs..be])
}

pub fn str_find(s: u64, sub: u64) -> u64 {
    let (hay, needle) = (str_ref(s), str_ref(sub));
    match hay.find(needle) {
        Some(bi) => {
            if str_is_ascii(s) {
                bi as u64
            } else {
                hay[..bi].chars().count() as u64
            }
        }
        None => (-1i64) as u64,
    }
}

pub fn str_contains(s: u64, sub: u64) -> u64 {
    b(str_ref(s).contains(str_ref(sub)))
}

pub fn str_replace(s: u64, from: u64, to: u64) -> u64 {
    let fr = str_ref(from);
    if fr.is_empty() {
        return s;
    }
    string(&str_ref(s).replace(fr, str_ref(to)))
}

pub fn str_split(s: u64, sep: u64) -> u64 {
    let (st, sp) = (str_ref(s), str_ref(sep));
    let parts: Vec<String> = if sp.is_empty() {
        st.chars().map(|c| c.to_string()).collect()
    } else {
        st.split(sp).map(|x| x.to_string()).collect()
    };
    array_of_strings(&parts)
}

pub fn str_trim(s: u64) -> u64 {
    string(str_ref(s).trim())
}

pub fn str_upper(s: u64) -> u64 {
    string(&str_ref(s).to_uppercase())
}

pub fn str_lower(s: u64) -> u64 {
    string(&str_ref(s).to_lowercase())
}

pub fn str_starts(s: u64, p: u64) -> u64 {
    b(str_bytes(s).starts_with(str_bytes(p)))
}

pub fn str_ends(s: u64, p: u64) -> u64 {
    b(str_bytes(s).ends_with(str_bytes(p)))
}

pub fn str_repeat(s: u64, n: u64) -> u64 {
    let n = (n as i64).max(0) as usize;
    string(&str_ref(s).repeat(n))
}

pub fn str_chars(s: u64) -> u64 {
    let parts: Vec<String> = str_ref(s).chars().map(|c| c.to_string()).collect();
    array_of_strings(&parts)
}

pub fn char_class(s: u64, kind: u64) -> u64 {
    let st = str_ref(s);
    if st.is_empty() {
        return 0;
    }
    b(st.chars().all(|c| match kind {
        0 => c.is_alphabetic(),
        1 => c.is_ascii_digit(),
        2 => c.is_alphanumeric(),
        _ => c.is_whitespace(),
    }))
}

pub fn str_code(s: u64) -> u64 {
    match str_ref(s).chars().next() {
        Some(c) => c as u64,
        None => (-1i64) as u64,
    }
}

pub fn str_from_code(n: u64) -> u64 {
    match char::from_u32(n as u32) {
        Some(c) => string(&c.to_string()),
        None => string(""),
    }
}

pub fn to_str(v: u64, tid: u64) -> u64 {
    if tid as u32 == TID_STR {
        return v;
    }
    string(&fmt::to_string(v, tid as u32))
}

pub fn parse_int(s: u64, loc: u64) -> u64 {
    let t = str_ref(s).trim();
    if let Ok(v) = t.parse::<i64>() {
        return v as u64;
    }
    if let Ok(v) = t.parse::<f64>() {
        if v.is_finite() {
            return (v.trunc() as i64) as u64;
        }
    }
    rt_error(&format!("cannot convert \"{}\" to int", t), loc)
}

pub fn parse_float(s: u64, loc: u64) -> u64 {
    let t = str_ref(s).trim();
    match t.parse::<f64>() {
        Ok(v) => fv(v),
        Err(_) => rt_error(&format!("cannot convert \"{}\" to float", t), loc),
    }
}

pub fn is_int_str(s: u64) -> u64 {
    b(str_ref(s).trim().parse::<i64>().is_ok())
}

pub fn is_float_str(s: u64) -> u64 {
    b(str_ref(s).trim().parse::<f64>().is_ok())
}

pub fn print(s: u64) -> u64 {
    let bytes = str_bytes(s);
    let mut v = Vec::with_capacity(bytes.len() + 1);
    v.extend_from_slice(bytes);
    v.push(b'\n');
    io::write_out(&v);
    0
}

pub fn print_err(s: u64) -> u64 {
    io::flush();
    use std::io::Write;
    let mut e = std::io::stderr().lock();
    let _ = e.write_all(str_bytes(s));
    let _ = e.write_all(b"\n");
    0
}

pub fn read_stdin() -> u64 {
    use std::io::Read;
    io::flush();
    let mut buf = Vec::new();
    let _ = std::io::stdin().lock().read_to_end(&mut buf);
    string(&String::from_utf8_lossy(&buf))
}

pub fn print_raw(s: u64) -> u64 {
    io::write_out(str_bytes(s));
    0
}

pub fn input(prompt: u64) -> u64 {
    let line = io::read_line(str_ref(prompt));
    string(&line)
}

pub fn arr_new(tid: u64, len: u64) -> u64 {
    array_new(tid as u32, len as usize)
}

pub fn arr_len(a: u64) -> u64 {
    array_len(a) as u64
}

pub fn arr_get(a: u64, i: u64, loc: u64) -> u64 {
    let n = array_len(a);
    if (i as usize) >= n {
        err_index(loc, i, n as u64);
    }
    unsafe { *array_data(a).add(i as usize) }
}

pub fn arr_set(a: u64, i: u64, v: u64, loc: u64) -> u64 {
    let n = array_len(a);
    if (i as usize) >= n {
        err_index(loc, i, n as u64);
    }
    unsafe { *array_data(a).add(i as usize) = v }
    v
}

pub fn arr_push(a: u64, v: u64) -> u64 {
    array_push(a, v);
    0
}

pub fn arr_pop(a: u64, loc: u64) -> u64 {
    let n = array_len(a);
    if n == 0 {
        rt_error("pop from empty array", loc);
    }
    let v = array_slice(a)[n - 1];
    unsafe { set_word(a, ARR_LEN, (n - 1) as u64) }
    v
}

pub fn arr_insert(a: u64, i: u64, v: u64, loc: u64) -> u64 {
    let n = array_len(a);
    let idx = i as i64;
    if idx < 0 || idx as usize > n {
        err_index(loc, i, n as u64);
    }
    array_set_len(a, n + 1);
    let s = array_slice_mut(a);
    s.copy_within(idx as usize..n, idx as usize + 1);
    s[idx as usize] = v;
    0
}

pub fn arr_remove(a: u64, i: u64, loc: u64) -> u64 {
    let n = array_len(a);
    if (i as usize) >= n {
        err_index(loc, i, n as u64);
    }
    let s = array_slice_mut(a);
    let v = s[i as usize];
    s.copy_within(i as usize + 1..n, i as usize);
    unsafe { set_word(a, ARR_LEN, (n - 1) as u64) }
    v
}

pub fn arr_concat(a: u64, c: u64) -> u64 {
    let mut v = Vec::with_capacity(array_len(a) + array_len(c));
    v.extend_from_slice(array_slice(a));
    v.extend_from_slice(array_slice(c));
    array_from(tid_of(a), &v)
}

pub fn arr_slice(a: u64, s: u64, e: u64) -> u64 {
    let n = array_len(a) as i64;
    let end = (e as i64).clamp(0, n);
    let start = (s as i64).clamp(0, end);
    let items: Vec<u64> = array_slice(a)[start as usize..end as usize].to_vec();
    array_from(tid_of(a), &items)
}

pub fn arr_copy(a: u64) -> u64 {
    let items: Vec<u64> = array_slice(a).to_vec();
    array_from(tid_of(a), &items)
}

pub fn arr_index_of(a: u64, v: u64, etid: u64) -> u64 {
    for (i, x) in array_slice(a).iter().enumerate() {
        if fmt::equals(*x, v, etid as u32) {
            return i as u64;
        }
    }
    (-1i64) as u64
}

pub fn arr_contains(a: u64, v: u64, etid: u64) -> u64 {
    b(arr_index_of(a, v, etid) as i64 >= 0)
}

pub fn arr_join(a: u64, sep: u64, etid: u64) -> u64 {
    let parts: Vec<String> = array_slice(a).iter().map(|x| fmt::to_string(*x, etid as u32)).collect();
    string(&parts.join(str_ref(sep)))
}

pub fn arr_reverse(a: u64) -> u64 {
    array_slice_mut(a).reverse();
    0
}

pub fn arr_sort(a: u64, etid: u64) -> u64 {
    let t = etid as u32;
    array_slice_mut(a).sort_by(|x, y| fmt::compare(*x, *y, t));
    0
}

pub fn arr_clear(a: u64) -> u64 {
    unsafe { set_word(a, ARR_LEN, 0) }
    0
}

fn map_key(m: u64, k: u64) -> MapKey {
    let kt = match desc(tid_of(m)) {
        Desc::Map(k, _) => *k,
        _ => TID_STR,
    };
    match desc(kt) {
        Desc::Str => MapKey::S(str_bytes(k).into()),
        Desc::Any => {
            if k != 0 && tid_of(k) == TID_STR {
                MapKey::S(str_bytes(box_val(k)).into())
            } else if k != 0 {
                MapKey::I(box_val(k))
            } else {
                MapKey::I(0)
            }
        }
        _ => MapKey::I(k),
    }
}

pub fn map_new_obj(tid: u64) -> u64 {
    map_new(tid as u32)
}

pub fn map_set(m: u64, k: u64, v: u64) -> u64 {
    let key = map_key(m, k);
    let d = map_data(m);
    match d.index.get(&key) {
        Some(i) => d.vals[*i] = v,
        None => {
            d.index.insert(key, d.keys.len());
            d.keys.push(k);
            d.vals.push(v);
        }
    }
    v
}

pub fn map_get(m: u64, k: u64, loc: u64) -> u64 {
    let key = map_key(m, k);
    let d = map_data(m);
    match d.index.get(&key) {
        Some(i) => d.vals[*i],
        None => {
            let kt = match desc(tid_of(m)) {
                Desc::Map(k, _) => *k,
                _ => TID_STR,
            };
            let mut ks = String::new();
            fmt::write(k, kt, &mut ks, true, 0);
            rt_error(&format!("key {} not found in map", ks), loc)
        }
    }
}

pub fn map_get_or(m: u64, k: u64, dflt: u64) -> u64 {
    let key = map_key(m, k);
    let d = map_data(m);
    match d.index.get(&key) {
        Some(i) => d.vals[*i],
        None => dflt,
    }
}

pub fn map_has(m: u64, k: u64) -> u64 {
    let key = map_key(m, k);
    b(map_data(m).index.contains_key(&key))
}

pub fn map_remove(m: u64, k: u64) -> u64 {
    let key = map_key(m, k);
    let d = map_data(m);
    match d.index.remove(&key) {
        Some(i) => {
            d.keys.remove(i);
            d.vals.remove(i);
            for v in d.index.values_mut() {
                if *v > i {
                    *v -= 1;
                }
            }
            1
        }
        None => 0,
    }
}

pub fn map_keys(m: u64, arr_tid: u64) -> u64 {
    let items = map_data(m).keys.clone();
    array_from(arr_tid as u32, &items)
}

pub fn map_values(m: u64, arr_tid: u64) -> u64 {
    let items = map_data(m).vals.clone();
    array_from(arr_tid as u32, &items)
}

pub fn map_len(m: u64) -> u64 {
    map_data(m).keys.len() as u64
}

pub fn struct_alloc(tid: u64, n: u64) -> u64 {
    struct_new(tid as u32, n as usize)
}

pub fn box_value(v: u64, tid: u64) -> u64 {
    let tid = tid as u32;
    match desc(tid) {
        Desc::Any => v,
        Desc::Null | Desc::Void => 0,
        Desc::Optional(inner) => {
            if v == 0 {
                0
            } else if meta::is_unboxed(*inner) {
                v
            } else {
                box_value(v, *inner as u64)
            }
        }
        Desc::Interface { .. } => {
            if v == 0 {
                0
            } else {
                box_raw(tid_of(v), v)
            }
        }
        Desc::Record { .. } => box_raw(tid_of(v), v),
        _ => box_raw(tid, v),
    }
}

fn dyn_matches(actual: u32, to: u32) -> bool {
    if actual == to {
        return true;
    }
    match desc(to) {
        Desc::Interface { .. } | Desc::Record { .. } => meta::implements(actual, to),
        Desc::Any => true,
        Desc::Optional(inner) => dyn_matches(actual, *inner),
        _ => false,
    }
}

pub fn is_type(v: u64, from: u64, to: u64) -> u64 {
    let (from, to) = (from as u32, to as u32);
    if matches!(desc(to), Desc::Null) {
        return b(v == 0);
    }
    match desc(from) {
        Desc::Any => b(v != 0 && dyn_matches(tid_of(v), to)),
        Desc::Optional(inner) => {
            if v == 0 {
                return 0;
            }
            if meta::is_unboxed(*inner) {
                b(dyn_matches(*inner, to))
            } else {
                is_type(v, *inner as u64, to as u64)
            }
        }
        Desc::Interface { .. } | Desc::Record { .. } => b(v != 0 && dyn_matches(tid_of(v), to)),
        _ => b(dyn_matches(from, to)),
    }
}

pub fn cast(v: u64, from: u64, to: u64, loc: u64) -> u64 {
    if is_type(v, from, to) == 0 {
        let actual = match desc(from as u32) {
            Desc::Any => {
                if v == 0 {
                    "null".to_string()
                } else {
                    meta::type_name(tid_of(v))
                }
            }
            Desc::Optional(_) if v == 0 => "null".to_string(),
            Desc::Interface { .. } | Desc::Record { .. } => meta::type_name(tid_of(v)),
            _ => meta::type_name(from as u32),
        };
        rt_error(&format!("cannot cast value of type {} to {}", actual, meta::type_name(to as u32)), loc);
    }
    payload(v, from as u32, to as u32)
}

fn payload(v: u64, from: u32, to: u32) -> u64 {
    match desc(from) {
        Desc::Any => match desc(to) {
            Desc::Any => v,
            Desc::Optional(inner) => {
                if meta::is_unboxed(*inner) {
                    v
                } else {
                    box_val(v)
                }
            }
            _ => box_val(v),
        },
        Desc::Optional(inner) => {
            if meta::is_unboxed(*inner) {
                box_val(v)
            } else {
                v
            }
        }
        _ => v,
    }
}

pub fn unwrap(v: u64, opt_tid: u64, loc: u64) -> u64 {
    if v == 0 {
        rt_error("unexpected null value", loc);
    }
    match desc(opt_tid as u32) {
        Desc::Optional(inner) if meta::is_unboxed(*inner) => box_val(v),
        _ => v,
    }
}

pub fn destroy(v: u64, loc: u64) -> u64 {
    if v == 0 {
        rt_error("cannot destroy null", loc);
    }
    match kind_of(v) {
        K_STRUCT => unsafe {
            for i in 0..struct_len(v) {
                set_word(v, HDR + 8 * i, 0);
            }
            hdr(v).kind = K_DEAD;
            0
        },
        K_DEAD => rt_error("this object was already destroyed", loc),
        _ => rt_error("only struct objects can be destroyed", loc),
    }
}

pub fn alive(v: u64, loc: u64) -> u64 {
    if v != 0 && kind_of(v) == K_DEAD {
        rt_error("this object was destroyed and can no longer be used", loc);
    }
    v
}

pub fn eq(a: u64, c: u64, tid: u64) -> u64 {
    b(fmt::equals(a, c, tid as u32))
}

pub fn cmp(a: u64, c: u64, tid: u64) -> u64 {
    match fmt::compare(a, c, tid as u32) {
        std::cmp::Ordering::Less => (-1i64) as u64,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

pub fn type_name(v: u64, tid: u64) -> u64 {
    let tid = tid as u32;
    let name = match desc(tid) {
        Desc::Any => {
            if v == 0 {
                "null".to_string()
            } else {
                meta::type_name(tid_of(v))
            }
        }
        Desc::Interface { .. } => meta::type_name(tid_of(v)),
        Desc::Optional(_) if v == 0 => "null".to_string(),
        Desc::Optional(inner) => meta::type_name(*inner),
        _ => meta::type_name(tid),
    };
    string(&name)
}

pub fn any_index(v: u64, key: u64, key_tid: u64, loc: u64) -> u64 {
    if v == 0 {
        rt_error("cannot index null", loc);
    }
    let inner = box_val(v);
    match desc(tid_of(v)) {
        Desc::Map(kt, vt) => {
            let (kt, vt) = (*kt, *vt);
            let raw_key = match desc(kt) {
                Desc::Any => box_value(key, key_tid),
                _ => match desc(key_tid as u32) {
                    Desc::Any => {
                        if key == 0 {
                            0
                        } else {
                            box_val(key)
                        }
                    }
                    _ => key,
                },
            };
            let k = map_key(inner, raw_key);
            let d = map_data(inner);
            match d.index.get(&k) {
                Some(i) => box_value(d.vals[*i], vt as u64),
                None => 0,
            }
        }
        Desc::Array(et) => {
            let et = *et;
            let i = match desc(key_tid as u32) {
                Desc::Int => key,
                Desc::Any if key != 0 && tid_of(key) == TID_INT => box_val(key),
                _ => rt_error("array index must be an int", loc),
            };
            let n = array_len(inner);
            if (i as usize) >= n {
                err_index(loc, i, n as u64);
            }
            box_value(array_slice(inner)[i as usize], et as u64)
        }
        Desc::Record { fields, .. } => {
            let name = match desc(key_tid as u32) {
                Desc::Str => str_ref(key).to_string(),
                Desc::Any if key != 0 && tid_of(key) == TID_STR => str_ref(box_val(key)).to_string(),
                _ => rt_error("record field name must be a string", loc),
            };
            match fields.iter().position(|(n, _)| *n == name) {
                Some(i) => box_value(field(inner, i), fields[i].1 as u64),
                None => 0,
            }
        }
        Desc::Str => {
            let i = match desc(key_tid as u32) {
                Desc::Int => key,
                _ => rt_error("string index must be an int", loc),
            };
            box_raw(TID_STR, str_index(inner, i, loc))
        }
        _ => rt_error(&format!("cannot index value of type {}", meta::type_name(tid_of(v))), loc),
    }
}

pub fn any_len(v: u64, loc: u64) -> u64 {
    if v == 0 {
        return 0;
    }
    let inner = box_val(v);
    match desc(tid_of(v)) {
        Desc::Map(..) => map_len(inner),
        Desc::Array(_) => array_len(inner) as u64,
        Desc::Str => str_len(inner),
        _ => rt_error(&format!("value of type {} has no length", meta::type_name(tid_of(v))), loc),
    }
}

pub fn fmath(op: u64, x: u64) -> u64 {
    let x = f(x);
    fv(match op {
        0 => x.sqrt(),
        1 => x.floor(),
        2 => x.ceil(),
        3 => x.round(),
        4 => x.abs(),
        5 => x.sin(),
        6 => x.cos(),
        7 => x.tan(),
        8 => x.ln(),
        9 => x.log10(),
        10 => x.exp(),
        11 => x.asin(),
        12 => x.acos(),
        13 => x.atan(),
        _ => x,
    })
}

pub fn fpow(a: u64, c: u64) -> u64 {
    fv(f(a).powf(f(c)))
}

pub fn fatan2(a: u64, c: u64) -> u64 {
    fv(f(a).atan2(f(c)))
}

pub fn fmod(a: u64, c: u64) -> u64 {
    fv(f(a) % f(c))
}

pub fn ipow(a: u64, e: u64) -> u64 {
    let (mut base, mut exp) = (a as i64, e as i64);
    if exp < 0 {
        return 0;
    }
    let mut r: i64 = 1;
    while exp > 0 {
        if exp & 1 == 1 {
            r = r.wrapping_mul(base);
        }
        base = base.wrapping_mul(base);
        exp >>= 1;
    }
    r as u64
}

static RNG: AtomicU64 = AtomicU64::new(0);

fn next_rand() -> u64 {
    let mut x = RNG.load(Ordering::Relaxed);
    if x == 0 {
        x = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(88172645463325252)
            | 1;
    }
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    RNG.store(x, Ordering::Relaxed);
    x
}

pub fn random() -> u64 {
    fv((next_rand() >> 11) as f64 / (1u64 << 53) as f64)
}

pub fn random_int(lo: u64, hi: u64) -> u64 {
    let (lo, hi) = (lo as i64, hi as i64);
    if hi <= lo {
        return lo as u64;
    }
    let span = (hi - lo) as u64 + 1;
    (lo + (next_rand() % span) as i64) as u64
}

pub fn seed(s: u64) -> u64 {
    RNG.store(s | 1, Ordering::Relaxed);
    0
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn now_sec() -> u64 {
    fv(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0))
}

pub fn clock_ns() -> u64 {
    use std::sync::OnceLock;
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_nanos() as u64
}

pub fn sleep_ms(ms: u64) -> u64 {
    io::flush();
    let ms = (ms as i64).max(0) as u64;
    std::thread::sleep(std::time::Duration::from_millis(ms));
    0
}

pub fn local_time() -> u64 {
    let parts = crate::time::local_parts();
    array_from(TID_ARR_INT, &parts.map(|x| x as u64))
}

pub fn http_request(method: u64, url: u64, body: u64, headers: u64) -> u64 {
    let hs: Vec<String> = array_slice(headers).iter().map(|h| str_ref(*h).to_string()).collect();
    let (status, rbody, rheaders) = crate::http::request(str_ref(method), str_ref(url), str_bytes(body), &hs);
    array_of_strings(&[status.to_string(), rbody, rheaders])
}

pub fn json_parse(s: u64, loc: u64) -> u64 {
    match crate::json::parse(str_ref(s)) {
        Ok(v) => v,
        Err(e) => rt_error(&format!("invalid JSON: {}", e), loc),
    }
}

pub fn json_stringify(v: u64, tid: u64) -> u64 {
    let mut out = String::new();
    crate::json::stringify(v, tid as u32, &mut out);
    string(&out)
}

pub fn read_file(path: u64, loc: u64) -> u64 {
    match std::fs::read(str_ref(path)) {
        Ok(b) => string(&String::from_utf8_lossy(&b)),
        Err(e) => rt_error(&format!("cannot read file \"{}\": {}", str_ref(path), e), loc),
    }
}

pub fn write_file(path: u64, content: u64) -> u64 {
    b(std::fs::write(str_ref(path), str_bytes(content)).is_ok())
}

pub fn append_file(path: u64, content: u64) -> u64 {
    use std::io::Write;
    let r = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(str_ref(path))
        .and_then(|mut f| f.write_all(str_bytes(content)));
    b(r.is_ok())
}

pub fn file_exists(path: u64) -> u64 {
    b(std::path::Path::new(str_ref(path)).exists())
}

pub fn env_var(name: u64) -> u64 {
    string(&std::env::var(str_ref(name)).unwrap_or_default())
}

pub fn args() -> u64 {
    array_of_strings(&io::args())
}

pub fn exit_now(code: u64) -> u64 {
    crate::task::wait_all();
    io::exit_now(code as i32)
}

pub fn panic(msg: u64, loc: u64) -> u64 {
    rt_error(str_ref(msg), loc)
}

pub fn assert(cond: u64, msg: u64, loc: u64) -> u64 {
    if cond == 0 {
        rt_error(&format!("assertion failed: {}", str_ref(msg)), loc);
    }
    0
}

pub fn err_index(loc: u64, idx: u64, len: u64) -> u64 {
    rt_error(&format!("index {} out of bounds (length {})", idx as i64, len), loc)
}

pub fn err_divzero(loc: u64) -> u64 {
    rt_error("division by zero", loc)
}

pub fn err_null(loc: u64) -> u64 {
    rt_error("unexpected null value", loc)
}

pub fn err_return(loc: u64) -> u64 {
    rt_error("function ended without returning a value", loc)
}

static TRAMPOLINE: AtomicUsize = AtomicUsize::new(0);

pub fn rt_init(meta_ptr: u64, meta_len: u64, globals: u64, nglobals: u64, stack_base: u64, trampoline: u64) -> u64 {
    let blob = unsafe { std::slice::from_raw_parts(meta_ptr as usize as *const u8, meta_len as usize) };
    meta::set_meta(meta::decode(blob));
    if nglobals > 0 {
        gc::add_root_range(globals as usize, nglobals as usize);
    }
    gc::set_stack_base(stack_base as usize);
    crate::signal::install(stack_base as usize);
    TRAMPOLINE.store(trampoline as usize, Ordering::SeqCst);
    0
}

pub fn set_args_native(argc: u64, argv: u64) -> u64 {
    let mut out = Vec::new();
    unsafe {
        let argv = argv as usize as *const *const std::ffi::c_char;
        for i in 1..argc as usize {
            let p = *argv.add(i);
            if !p.is_null() {
                out.push(std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned());
            }
        }
    }
    io::set_args(out);
    0
}

pub fn rt_exit(code: u64) -> u64 {
    crate::task::wait_all();
    io::flush();
    code
}

pub fn trampoline() -> usize {
    TRAMPOLINE.load(Ordering::SeqCst)
}

pub fn spawn_native(fnptr: u64, argc: u64, argsptr: u64, tid: u64) -> u64 {
    let args: Vec<u64> = unsafe { std::slice::from_raw_parts(argsptr as usize as *const u64, argc as usize).to_vec() };
    let tramp = TRAMPOLINE.load(Ordering::SeqCst);
    crate::task::spawn(
        tid as u32,
        Box::new(move || {
            let t: extern "C" fn(u64, u64, u64) -> u64 = unsafe { std::mem::transmute(tramp) };
            t(fnptr, args.as_ptr() as u64, args.len() as u64)
        }),
    )
}

pub fn await_future(fut: u64) -> u64 {
    crate::task::await_future(fut)
}

pub fn gc_collect() -> u64 {
    gc::force_collect();
    0
}

pub fn iabs(a: u64) -> u64 {
    (a as i64).wrapping_abs() as u64
}

pub fn imin(a: u64, c: u64) -> u64 {
    (a as i64).min(c as i64) as u64
}

pub fn imax(a: u64, c: u64) -> u64 {
    (a as i64).max(c as i64) as u64
}

pub fn fmin(a: u64, c: u64) -> u64 {
    fv(f(a).min(f(c)))
}

pub fn fmax(a: u64, c: u64) -> u64 {
    fv(f(a).max(f(c)))
}
