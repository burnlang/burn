use crate::check::{self, CheckOptions};
use crate::diag::{self, Diagnostic};
use crate::driver;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

type FileEdits = HashMap<PathBuf, (String, Vec<(usize, usize, String)>)>;

fn collect(file: &Path) -> Result<(FileEdits, usize), String> {
    let loaded = driver::load_path(file)?;
    let result = check::check(&loaded, CheckOptions::default());
    let diags: Vec<&Diagnostic> = loaded.diags.iter().chain(result.diags.iter()).collect();
    let mut out: FileEdits = HashMap::new();
    let mut count = 0;
    for d in diags {
        let Some(s) = d.suggestions.iter().find(|s| s.applicable) else { continue };
        let Some(first) = s.edits.first() else { continue };
        let f = loaded.sm.file(first.0.file);
        let Some(path) = f.path.clone() else { continue };
        if s.edits.iter().any(|(sp, _)| sp.file != first.0.file) {
            continue;
        }
        let entry = out.entry(path).or_insert_with(|| (f.src.clone(), Vec::new()));
        let overlaps = s.edits.iter().any(|(sp, _)| {
            let (a2, b2) = (sp.start as usize, sp.end as usize);
            entry.1.iter().any(|(a, b, _)| (a2 < *b && *a < b2) || a2 == *a)
        });
        if overlaps {
            continue;
        }
        for (sp, rep) in &s.edits {
            entry.1.push((sp.start as usize, sp.end as usize, rep.clone()));
        }
        count += 1;
    }
    Ok((out, count))
}

pub fn cmd(args: &[String]) -> ExitCode {
    let dry = args.iter().any(|a| a == "--dry-run");
    let files: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    if files.is_empty() {
        eprintln!("error: no source file given");
        return ExitCode::from(2);
    }
    let mut failed = false;
    for f in files {
        let path = Path::new(f);
        let mut fixed = 0;
        let mut touched: Vec<PathBuf> = Vec::new();
        for _ in 0..8 {
            let (edits, count) = match collect(path) {
                Ok(x) => x,
                Err(e) => {
                    eprintln!("error: {}", e);
                    failed = true;
                    break;
                }
            };
            if count == 0 {
                break;
            }
            fixed += count;
            for (p, (src, es)) in edits {
                let patched = diag::apply(&src, &es);
                if dry {
                    println!("would fix {} in {}", es.len(), p.display());
                } else if let Err(e) = std::fs::write(&p, patched) {
                    eprintln!("error: cannot write {}: {}", p.display(), e);
                    failed = true;
                }
                if !touched.contains(&p) {
                    touched.push(p);
                }
            }
            if dry {
                break;
            }
        }
        if fixed > 0 {
            let names: Vec<String> = touched.iter().map(|p| p.display().to_string()).collect();
            eprintln!(
                "{} {} problem{} in {}",
                if dry { "can fix" } else { "fixed" },
                fixed,
                if fixed == 1 { "" } else { "s" },
                names.join(", ")
            );
        }
        match driver::compile_path(path) {
            Ok(c) => driver::report(&c.sm, &c.warnings),
            Err(e) => {
                driver::report(&e.sm, &e.diags);
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
