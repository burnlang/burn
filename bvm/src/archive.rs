use crate::binary;
use crate::link::{link_with, LinkOptions};
use crate::module::Module;

pub const MAGIC: &[u8; 4] = b"BAR\0";
pub const VERSION: u16 = 1;
pub const SHEBANG: &str = "#!/usr/bin/env bvm\n";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub main: String,
    pub entry: Option<String>,
    pub created_by: String,
    pub extra: Vec<(String, String)>,
}

impl Manifest {
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        let mut line = |k: &str, v: &str| {
            if !v.is_empty() {
                s.push_str(k);
                s.push_str(": ");
                s.push_str(v);
                s.push('\n');
            }
        };
        line("Name", &self.name);
        line("Version", &self.version);
        line("Main-Module", &self.main);
        line("Entry", self.entry.as_deref().unwrap_or(""));
        line("Created-By", &self.created_by);
        for (k, v) in &self.extra {
            line(k, v);
        }
        s
    }

    pub fn parse(text: &str) -> Manifest {
        let mut m = Manifest::default();
        for l in text.lines() {
            let Some((k, v)) = l.split_once(':') else { continue };
            let v = v.trim().to_string();
            match k.trim() {
                "Name" => m.name = v,
                "Version" => m.version = v,
                "Main-Module" => m.main = v,
                "Entry" => m.entry = Some(v),
                "Created-By" => m.created_by = v,
                other => m.extra.push((other.to_string(), v)),
            }
        }
        m
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Archive {
    pub manifest: Manifest,
    pub modules: Vec<(String, Module)>,
    pub resources: Vec<(String, Vec<u8>)>,
}

pub fn is_archive(bytes: &[u8]) -> bool {
    payload_start(bytes).is_some()
}

fn payload_start(bytes: &[u8]) -> Option<usize> {
    if bytes.starts_with(MAGIC) {
        return Some(0);
    }
    if bytes.starts_with(b"#!") {
        let nl = bytes.iter().position(|b| *b == b'\n')?;
        if bytes[nl + 1..].starts_with(MAGIC) {
            return Some(nl + 1);
        }
    }
    None
}

fn checksum(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for x in b {
        h ^= *x as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

struct Rd<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Rd<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.b.len() - self.i < n {
            return Err("the archive ends early".into());
        }
        let s = &self.b[self.i..self.i + n];
        self.i += n;
        Ok(s)
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn bytes(&mut self) -> Result<&'a [u8], String> {
        let n = self.u32()? as usize;
        self.take(n)
    }
    fn s(&mut self) -> Result<String, String> {
        String::from_utf8(self.bytes()?.to_vec()).map_err(|_| "invalid UTF-8 in the archive".to_string())
    }
}

impl Archive {
    pub fn host(&self) -> crate::Host {
        let mut host = crate::Host::new();
        let resources = std::sync::Arc::new(self.resources.clone());
        host.register("resource", 1, move |a| {
            let name = bvm_runtime::obj::str_ref(a[0]).to_string();
            match resources.iter().find(|(n, _)| *n == name) {
                Some((_, data)) => bvm_runtime::obj::string(&String::from_utf8_lossy(data)),
                None => 0,
            }
        });
        host
    }

    pub fn new(name: &str) -> Archive {
        Archive {
            manifest: Manifest {
                name: name.to_string(),
                created_by: format!("bvm {}", crate::VERSION),
                ..Manifest::default()
            },
            ..Archive::default()
        }
    }

    pub fn add_module(&mut self, name: &str, m: Module) {
        if self.manifest.main.is_empty() {
            self.manifest.main = name.to_string();
        }
        self.modules.retain(|(n, _)| n != name);
        self.modules.push((name.to_string(), m));
    }

    pub fn add_resource(&mut self, name: &str, data: Vec<u8>) {
        self.resources.retain(|(n, _)| n != name);
        self.resources.push((name.to_string(), data));
    }

    pub fn resource(&self, name: &str) -> Option<&[u8]> {
        self.resources.iter().find(|(n, _)| n == name).map(|(_, d)| d.as_slice())
    }

    pub fn encode(&self, shebang: bool) -> Vec<u8> {
        let mut body = Vec::new();
        let put = |b: &mut Vec<u8>, data: &[u8]| {
            b.extend_from_slice(&(data.len() as u32).to_le_bytes());
            b.extend_from_slice(data);
        };
        put(&mut body, self.manifest.to_text().as_bytes());
        body.extend_from_slice(&((self.modules.len() + self.resources.len()) as u32).to_le_bytes());
        for (name, m) in &self.modules {
            put(&mut body, name.as_bytes());
            body.push(0);
            put(&mut body, &binary::encode(m));
        }
        for (name, data) in &self.resources {
            put(&mut body, name.as_bytes());
            body.push(1);
            put(&mut body, data);
        }
        let mut out = Vec::with_capacity(body.len() + 32);
        if shebang {
            out.extend_from_slice(SHEBANG.as_bytes());
        }
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&checksum(&body).to_le_bytes());
        out.extend_from_slice(&body);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Archive, String> {
        let start = payload_start(bytes).ok_or("not a bvm archive (it does not start with BAR\\0)")?;
        let mut r = Rd { b: bytes, i: start + 4 };
        let version = u16::from_le_bytes(r.take(2)?.try_into().unwrap());
        if version != VERSION {
            return Err(format!("the archive uses format {}, but this bvm reads format {}", version, VERSION));
        }
        r.take(2)?;
        let sum = u64::from_le_bytes(r.take(8)?.try_into().unwrap());
        if checksum(&bytes[r.i..]) != sum {
            return Err("the archive is damaged (its checksum does not match)".into());
        }
        let manifest = Manifest::parse(&r.s()?);
        let n = r.u32()? as usize;
        let mut a = Archive {
            manifest,
            ..Archive::default()
        };
        for _ in 0..n {
            let name = r.s()?;
            let kind = r.take(1)?[0];
            let data = r.bytes()?;
            match kind {
                0 => a
                    .modules
                    .push((name.clone(), binary::decode(data).map_err(|e| format!("module {}: {}", name, e))?)),
                1 => a.resources.push((name, data.to_vec())),
                k => return Err(format!("unknown archive entry kind {}", k)),
            }
        }
        if r.i != bytes.len() {
            return Err("unexpected bytes at the end of the archive".into());
        }
        Ok(a)
    }

    pub fn ordered(&self) -> Vec<Module> {
        let mut out: Vec<Module> = Vec::with_capacity(self.modules.len());
        if let Some((_, m)) = self.modules.iter().find(|(n, _)| *n == self.manifest.main) {
            out.push(m.clone());
        }
        for (n, m) in &self.modules {
            if *n != self.manifest.main {
                out.push(m.clone());
            }
        }
        out
    }

    pub fn link(&self) -> Result<Module, String> {
        self.link_with(&LinkOptions::default())
    }

    pub fn link_with(&self, opts: &LinkOptions) -> Result<Module, String> {
        let mods = self.ordered();
        if mods.is_empty() {
            return Err("the archive contains no modules".into());
        }
        let mut m = link_with(&mods, opts)?;
        if let Some(e) = &self.manifest.entry {
            let f = m
                .func(e)
                .ok_or_else(|| format!("the manifest names entry function {}, which does not exist", e))?;
            if m.funcs[f as usize].params != 0 {
                return Err(format!("entry function {} must not take parameters", e));
            }
            m.entry = Some(f);
        }
        Ok(m)
    }
}
