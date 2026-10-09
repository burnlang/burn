use crate::fmt::{float_str, quote};
use crate::meta::{desc, Desc, TID_ARR_ANY, TID_BOOL, TID_FLOAT, TID_INT, TID_MAP_STR_ANY, TID_STR};
use crate::obj::*;

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\n' | b'\r' | b'\t') {
            self.i += 1;
        }
    }

    fn err<T>(&self, m: &str) -> Result<T, String> {
        Err(format!("{} at offset {}", m, self.i))
    }

    fn lit(&mut self, w: &str) -> bool {
        if self.s[self.i..].starts_with(w.as_bytes()) {
            self.i += w.len();
            true
        } else {
            false
        }
    }

    fn value(&mut self) -> Result<u64, String> {
        self.ws();
        if self.i >= self.s.len() {
            return self.err("unexpected end of input");
        }
        let c0 = self.s[self.i];
        match c0 {
            b'{' => {
                self.i += 1;
                let m = map_new(crate::meta::TID_MAP_STR_ANY);
                self.ws();
                if self.i < self.s.len() && self.s[self.i] == b'}' {
                    self.i += 1;
                    return Ok(box_raw(TID_MAP_STR_ANY, m));
                }
                loop {
                    self.ws();
                    if self.i >= self.s.len() || self.s[self.i] != b'"' {
                        return self.err("expected string key");
                    }
                    let k = self.string()?;
                    let ko = string(&k);
                    self.ws();
                    if self.i >= self.s.len() || self.s[self.i] != b':' {
                        return self.err("expected ':'");
                    }
                    self.i += 1;
                    let v = self.value()?;
                    crate::api::map_insert_owned(m, ko, v);
                    self.ws();
                    if self.i < self.s.len() && self.s[self.i] == b',' {
                        self.i += 1;
                        continue;
                    }
                    if self.i < self.s.len() && self.s[self.i] == b'}' {
                        self.i += 1;
                        break;
                    }
                    return self.err("expected ',' or '}'");
                }
                Ok(box_raw(TID_MAP_STR_ANY, m))
            }
            b'[' => {
                self.i += 1;
                let a = array_new(TID_ARR_ANY, 0);
                self.ws();
                if self.i < self.s.len() && self.s[self.i] == b']' {
                    self.i += 1;
                    return Ok(box_raw(TID_ARR_ANY, a));
                }
                loop {
                    let v = self.value()?;
                    array_push(a, v);
                    self.ws();
                    if self.i < self.s.len() && self.s[self.i] == b',' {
                        self.i += 1;
                        continue;
                    }
                    if self.i < self.s.len() && self.s[self.i] == b']' {
                        self.i += 1;
                        break;
                    }
                    return self.err("expected ',' or ']'");
                }
                Ok(box_raw(TID_ARR_ANY, a))
            }
            b'"' => {
                let s = self.string()?;
                let so = string(&s);
                Ok(box_raw(TID_STR, so))
            }
            b't' if self.lit("true") => Ok(box_raw(TID_BOOL, 1)),
            b'f' if self.lit("false") => Ok(box_raw(TID_BOOL, 0)),
            b'n' if self.lit("null") => Ok(0),
            c if c == b'-' || c.is_ascii_digit() => {
                let st = self.i;
                self.i += 1;
                let mut float = false;
                while self.i < self.s.len() {
                    let c = self.s[self.i];
                    if c.is_ascii_digit() {
                        self.i += 1;
                    } else if matches!(c, b'.' | b'e' | b'E' | b'+' | b'-') {
                        float = true;
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                let t = std::str::from_utf8(&self.s[st..self.i]).unwrap_or("0");
                if !float {
                    if let Ok(v) = t.parse::<i64>() {
                        return Ok(box_raw(TID_INT, v as u64));
                    }
                }
                match t.parse::<f64>() {
                    Ok(v) => Ok(box_raw(TID_FLOAT, v.to_bits())),
                    Err(_) => self.err("invalid number"),
                }
            }
            _ => self.err("unexpected character"),
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.i += 1;
        let mut out: Vec<u8> = Vec::new();
        while self.i < self.s.len() {
            let c = self.s[self.i];
            self.i += 1;
            match c {
                b'"' => return Ok(String::from_utf8_lossy(&out).into_owned()),
                b'\\' => {
                    if self.i >= self.s.len() {
                        break;
                    }
                    let e = self.s[self.i];
                    self.i += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'r' => out.push(b'\r'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'u' => {
                            let mut cp = self.hex4()?;
                            if (0xD800..0xDC00).contains(&cp) && self.s[self.i..].starts_with(b"\\u") {
                                self.i += 2;
                                let lo = self.hex4()?;
                                cp = 0x10000 + ((cp - 0xD800) << 10) + (lo.wrapping_sub(0xDC00) & 0x3FF);
                            }
                            let ch = char::from_u32(cp).unwrap_or('\u{FFFD}');
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                        other => out.push(other),
                    }
                }
                _ => out.push(c),
            }
        }
        self.err("unterminated string")
    }

    fn hex4(&mut self) -> Result<u32, String> {
        if self.i + 4 > self.s.len() {
            return self.err("invalid unicode escape");
        }
        let h = std::str::from_utf8(&self.s[self.i..self.i + 4]).unwrap_or("");
        self.i += 4;
        u32::from_str_radix(h, 16).map_err(|_| "invalid unicode escape".to_string())
    }
}

pub fn parse(s: &str) -> Result<u64, String> {
    let mut p = P { s: s.as_bytes(), i: 0 };
    let v = p.value()?;
    p.ws();
    if p.i < p.s.len() {
        return p.err("trailing characters");
    }
    Ok(v)
}

pub fn stringify(v: u64, tid: u32, out: &mut String) {
    match desc(tid) {
        Desc::Int => out.push_str(&(v as i64).to_string()),
        Desc::Float => {
            let f = f64::from_bits(v);
            if f.is_finite() {
                out.push_str(&float_str(f))
            } else {
                out.push_str("null")
            }
        }
        Desc::Num(n) => {
            let text = crate::fmt::num_str(v, *n);
            if *n == crate::meta::Num::F32 && !(f64::from_bits(v)).is_finite() {
                out.push_str("null")
            } else {
                out.push_str(&text)
            }
        }
        Desc::Bool => out.push_str(if v != 0 { "true" } else { "false" }),
        Desc::Str => quote(str_ref(v), out),
        Desc::Enum { variants, .. } => quote(variants.get(v as usize).map(|s| s.as_str()).unwrap_or(""), out),
        Desc::Any => {
            if v == 0 {
                out.push_str("null")
            } else {
                stringify(box_val(v), tid_of(v), out)
            }
        }
        Desc::Optional(inner) => {
            if v == 0 {
                out.push_str("null")
            } else if crate::meta::is_unboxed(*inner) {
                stringify(box_val(v), *inner, out)
            } else {
                stringify(v, *inner, out)
            }
        }
        Desc::Array(e) => {
            out.push('[');
            for (i, x) in array_values(v).iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                stringify(*x, *e, out);
            }
            out.push(']');
        }
        Desc::Map(k, vt) => {
            let d = map_data(v);
            out.push('{');
            for i in 0..d.keys.len() {
                if i > 0 {
                    out.push(',');
                }
                let ks = crate::fmt::to_string(d.keys[i], *k);
                quote(&ks, out);
                out.push(':');
                stringify(d.vals[i], *vt, out);
            }
            out.push('}');
        }
        Desc::Record { fields, .. } => {
            if v != 0 && kind_of(v) == K_DEAD {
                out.push_str("null");
                return;
            }
            let real = tid_of(v);
            if real != tid {
                if let Desc::Record { .. } = desc(real) {
                    return stringify(v, real, out);
                }
            }
            out.push('{');
            for (i, (n, ft)) in fields.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                quote(n, out);
                out.push(':');
                stringify(field(v, i), *ft, out);
            }
            out.push('}');
        }
        Desc::Interface { .. } => {
            if v == 0 {
                out.push_str("null")
            } else {
                stringify(v, tid_of(v), out)
            }
        }
        _ => out.push_str("null"),
    }
}
