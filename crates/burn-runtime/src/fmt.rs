use crate::meta::{desc, Desc};
use crate::obj::*;

pub fn float_str(f: f64) -> String {
    if f.is_nan() {
        return "NaN".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if f.fract() == 0.0 && f.abs() < 1e16 {
        return format!("{:.1}", f);
    }
    if f.abs() >= 1e16 || (f != 0.0 && f.abs() < 1e-6) {
        return format!("{:e}", f);
    }
    format!("{}", f)
}

pub fn quote(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

pub fn to_string(v: u64, tid: u32) -> String {
    let mut s = String::new();
    write(v, tid, &mut s, false, 0);
    s
}

pub fn write(v: u64, tid: u32, out: &mut String, nested: bool, depth: usize) {
    if depth > 64 {
        out.push_str("...");
        return;
    }
    match desc(tid) {
        Desc::Int => out.push_str(&(v as i64).to_string()),
        Desc::Float => out.push_str(&float_str(f64::from_bits(v))),
        Desc::Bool => out.push_str(if v != 0 { "true" } else { "false" }),
        Desc::Str => {
            if nested {
                quote(str_ref(v), out)
            } else {
                out.push_str(str_ref(v))
            }
        }
        Desc::Void | Desc::Null => out.push_str("null"),
        Desc::Error => out.push_str("<error>"),
        Desc::Func => out.push_str("<fun>"),
        Desc::Future(_) => out.push_str("<future>"),
        Desc::Any => {
            if v == 0 {
                out.push_str("null")
            } else {
                write(box_val(v), tid_of(v), out, nested, depth + 1)
            }
        }
        Desc::Optional(inner) => {
            if v == 0 {
                out.push_str("null")
            } else if crate::meta::is_unboxed(*inner) {
                write(box_val(v), *inner, out, nested, depth + 1)
            } else {
                write(v, *inner, out, nested, depth + 1)
            }
        }
        Desc::Enum { variants, .. } => match variants.get(v as usize) {
            Some(n) => out.push_str(n),
            None => out.push_str(&(v as i64).to_string()),
        },
        Desc::Array(e) => {
            out.push('[');
            for (i, x) in array_slice(v).iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write(*x, *e, out, true, depth + 1);
            }
            out.push(']');
        }
        Desc::Map(k, val) => {
            let d = map_data(v);
            out.push('{');
            for i in 0..d.keys.len() {
                if i > 0 {
                    out.push_str(", ");
                }
                write(d.keys[i], *k, out, true, depth + 1);
                out.push_str(": ");
                write(d.vals[i], *val, out, true, depth + 1);
            }
            out.push('}');
        }
        Desc::Interface { .. } => {
            if v == 0 {
                out.push_str("null")
            } else {
                write(v, tid_of(v), out, nested, depth)
            }
        }
        Desc::Record { name, fields, .. } => {
            let real = tid_of(v);
            if real != tid {
                if let Desc::Record { .. } = desc(real) {
                    return write(v, real, out, nested, depth);
                }
            }
            if !name.is_empty() {
                out.push_str(name);
                out.push(' ');
            }
            out.push('{');
            for (i, (fname, ft)) in fields.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push(' ');
                out.push_str(fname);
                out.push_str(": ");
                write(field(v, i), *ft, out, true, depth + 1);
            }
            if !fields.is_empty() {
                out.push(' ');
            }
            out.push('}');
        }
    }
}

pub fn equals(a: u64, b: u64, tid: u32) -> bool {
    if a == b {
        return match desc(tid) {
            Desc::Float => !f64::from_bits(a).is_nan(),
            _ => true,
        };
    }
    match desc(tid) {
        Desc::Float => f64::from_bits(a) == f64::from_bits(b),
        Desc::Int | Desc::Bool | Desc::Enum { .. } | Desc::Func | Desc::Void | Desc::Null => false,
        Desc::Str => str_bytes(a) == str_bytes(b),
        Desc::Any => {
            if a == 0 || b == 0 {
                return false;
            }
            let (ta, tb) = (tid_of(a), tid_of(b));
            if ta != tb {
                let na = matches!(desc(ta), Desc::Int | Desc::Float);
                let nb = matches!(desc(tb), Desc::Int | Desc::Float);
                if na && nb {
                    return as_f64(box_val(a), ta) == as_f64(box_val(b), tb);
                }
                return false;
            }
            equals(box_val(a), box_val(b), ta)
        }
        Desc::Optional(inner) => {
            if a == 0 || b == 0 {
                return false;
            }
            if crate::meta::is_unboxed(*inner) {
                equals(box_val(a), box_val(b), *inner)
            } else {
                equals(a, b, *inner)
            }
        }
        Desc::Array(e) => {
            let (x, y) = (array_slice(a), array_slice(b));
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(p, q)| equals(*p, *q, *e))
        }
        Desc::Map(_, vt) => {
            let (x, y) = (map_data(a), map_data(b));
            if x.keys.len() != y.keys.len() {
                return false;
            }
            for (k, i) in x.index.iter() {
                match y.index.get(k) {
                    Some(j) => {
                        if !equals(x.vals[*i], y.vals[*j], *vt) {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
            true
        }
        Desc::Record { fields, .. } => {
            if a == 0 || b == 0 || tid_of(a) != tid_of(b) {
                return false;
            }
            fields.iter().enumerate().all(|(i, (_, ft))| equals(field(a, i), field(b, i), *ft))
        }
        Desc::Interface { .. } => {
            if a == 0 || b == 0 || tid_of(a) != tid_of(b) {
                return false;
            }
            equals(a, b, tid_of(a))
        }
        Desc::Future(_) | Desc::Error => false,
    }
}

pub fn as_f64(v: u64, tid: u32) -> f64 {
    match desc(tid) {
        Desc::Float => f64::from_bits(v),
        _ => v as i64 as f64,
    }
}

pub fn compare(a: u64, b: u64, tid: u32) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    match desc(tid) {
        Desc::Int | Desc::Enum { .. } => (a as i64).cmp(&(b as i64)),
        Desc::Float => f64::from_bits(a).partial_cmp(&f64::from_bits(b)).unwrap_or(Equal),
        Desc::Bool => a.cmp(&b),
        Desc::Str => str_bytes(a).cmp(str_bytes(b)),
        Desc::Any => {
            if a == 0 || b == 0 {
                return a.cmp(&b);
            }
            let (ta, tb) = (tid_of(a), tid_of(b));
            if ta == tb {
                compare(box_val(a), box_val(b), ta)
            } else {
                as_f64(box_val(a), ta).partial_cmp(&as_f64(box_val(b), tb)).unwrap_or(Equal)
            }
        }
        _ => Equal,
    }
}
