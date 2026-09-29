use std::path::PathBuf;

pub type FileId = u32;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Span {
    pub file: FileId,
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(file: FileId, start: usize, end: usize) -> Span {
        Span { file, start: start as u32, end: end as u32 }
    }

    pub fn to(self, other: Span) -> Span {
        if other.file != self.file {
            return self;
        }
        Span { file: self.file, start: self.start.min(other.start), end: self.end.max(other.end) }
    }

    pub fn contains(&self, offset: usize) -> bool {
        (self.start as usize) <= offset && offset <= (self.end as usize)
    }
}

pub struct SourceFile {
    pub name: String,
    pub path: Option<PathBuf>,
    pub src: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    pub fn new(name: String, path: Option<PathBuf>, src: String) -> SourceFile {
        let mut line_starts = vec![0];
        for (i, b) in src.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        SourceFile { name, path, src, line_starts }
    }

    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.src.len());
        let line = match self.line_starts.binary_search(&offset) {
            Ok(l) => l,
            Err(l) => l - 1,
        };
        let start = self.line_starts[line];
        let col = self.src[start..offset].chars().count();
        (line + 1, col + 1)
    }

    pub fn line_utf16_col(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.src.len());
        let line = match self.line_starts.binary_search(&offset) {
            Ok(l) => l,
            Err(l) => l - 1,
        };
        let start = self.line_starts[line];
        let col: usize = self.src[start..offset].chars().map(|c| c.len_utf16()).sum();
        (line, col)
    }

    pub fn offset_of_utf16(&self, line: usize, col: usize) -> usize {
        if line >= self.line_starts.len() {
            return self.src.len();
        }
        let start = self.line_starts[line];
        let mut units = 0;
        for (i, c) in self.src[start..].char_indices() {
            if units >= col || c == '\n' {
                return start + i;
            }
            units += c.len_utf16();
        }
        self.src.len()
    }

    pub fn line_text(&self, line: usize) -> &str {
        let start = self.line_starts[line - 1];
        let end = self.line_starts.get(line).map(|e| e - 1).unwrap_or(self.src.len());
        self.src[start..end.max(start)].trim_end_matches('\r')
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }
}

#[derive(Default)]
pub struct SourceMap {
    pub files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn add(&mut self, name: String, path: Option<PathBuf>, src: String) -> FileId {
        self.files.push(SourceFile::new(name, path, src));
        (self.files.len() - 1) as FileId
    }

    pub fn file(&self, id: FileId) -> &SourceFile {
        &self.files[id as usize]
    }

    pub fn location(&self, span: Span) -> String {
        let f = self.file(span.file);
        let (l, c) = f.line_col(span.start as usize);
        format!("{}:{}:{}", f.name, l, c)
    }
}
