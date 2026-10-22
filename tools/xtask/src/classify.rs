//! Sorts the files of a data folder into what `rerac2-extract` writes, known leftovers (an interrupted extract,
//! Finder's files), and unknown files (which `regen-data` refuses to delete without `--force`). Names only; nothing is read or written.

use std::collections::HashSet;
use std::path::Path;

/// What the extractor writes besides the Tier 0 table's files: the manifest, the Tier 1 cache, Tier 2 exports.
pub const INFO_FILE: &str = "extract-info.json";
pub const EXTRACTOR_DIRS: &[&str] = &["cache/", "exports/"];

/// The known stale leftovers, in match order (first match wins); `regen-data` removes them without asking.
pub const LEFTOVER_KINDS: &[&str] = &["*.partial (an interrupted extract)", ".DS_Store (Finder)"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// A Tier 0 file, `extract-info.json`, or under `cache/` / `exports/`.
    Extractor,
    /// A known leftover: index into [`LEFTOVER_KINDS`].
    Leftover(usize),
    Unknown,
}

/// Classifies one file by its path relative to the data folder (`/`-separated). `tier0` holds the Tier 0 paths.
pub fn classify(rel: &str, tier0: &HashSet<String>) -> Class {
    if tier0.contains(rel) || rel == INFO_FILE || EXTRACTOR_DIRS.iter().any(|d| rel.starts_with(d)) {
        return Class::Extractor;
    }
    let name = rel.rsplit('/').next().unwrap_or(rel);
    let kind = if name.ends_with(".partial") {
        0
    } else if name == ".DS_Store" {
        1
    } else {
        return Class::Unknown;
    };
    Class::Leftover(kind)
}

/// Tier 0 paths from every `*.tsv` table in `data_dir` (`crates/rc-extract/data`): the first column of each row.
pub fn load_tier0(data_dir: &Path) -> std::io::Result<HashSet<String>> {
    let mut set = HashSet::new();
    for e in std::fs::read_dir(data_dir)? {
        let p = e?.path();
        if p.extension().is_some_and(|x| x == "tsv") {
            for line in std::fs::read_to_string(&p)?.lines() {
                if line.starts_with('#') || line.trim().is_empty() { continue; }
                if let Some(path) = line.split('\t').next() { set.insert(path.to_string()); }
            }
        }
    }
    Ok(set)
}

#[derive(Default, Debug)]
pub struct Tally {
    pub files: u64,
    pub bytes: u64,
}

impl Tally {
    fn add(&mut self, bytes: u64) { self.files += 1; self.bytes += bytes; }
}

/// A data folder's contents by class.
#[derive(Default, Debug)]
pub struct Report {
    pub extractor: Tally,
    pub leftovers: Vec<Tally>,
    /// Unknown files (relative paths) and their total.
    pub unknown: Vec<String>,
    pub unknown_tally: Tally,
}

impl Report {
    pub fn leftover_total(&self) -> Tally {
        self.leftovers.iter().fold(Tally::default(), |mut t, l| { t.files += l.files; t.bytes += l.bytes; t })
    }
}

/// Walks `root` (symlinks are not followed and count as unknown files). Empty folders are ignored.
pub fn scan(root: &Path, tier0: &HashSet<String>) -> std::io::Result<Report> {
    let mut r = Report { leftovers: LEFTOVER_KINDS.iter().map(|_| Tally::default()).collect(), ..Report::default() };
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir)? {
            let e = e?;
            let ft = e.file_type()?;
            let path = e.path();
            if ft.is_dir() { stack.push(path); continue; }
            let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            let bytes = e.metadata().map(|m| m.len()).unwrap_or(0);
            match if ft.is_file() { classify(&rel, tier0) } else { Class::Unknown } {
                Class::Extractor => r.extractor.add(bytes),
                Class::Leftover(k) => r.leftovers[k].add(bytes),
                Class::Unknown => { r.unknown_tally.add(bytes); r.unknown.push(rel); }
            }
        }
    }
    r.unknown.sort();
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t0() -> HashSet<String> { ["RC2.HDR", "SCUS_972.68", "G/LEVEL0.WAD"].iter().map(|s| s.to_string()).collect() }

    #[test]
    fn extractor_output_is_recognised() {
        let t = t0();
        for p in ["RC2.HDR", "G/LEVEL0.WAD", "extract-info.json", "cache/v1/stamp.toml", "exports/textures/a.png"] {
            assert_eq!(classify(p, &t), Class::Extractor, "{p}");
        }
    }

    #[test]
    fn leftovers_are_known() {
        let t = t0();
        for (p, k) in [("G/LEVEL0.WAD.partial", 0), ("G/.DS_Store", 1), (".DS_Store", 1)] { assert_eq!(classify(p, &t), Class::Leftover(k), "{p}"); }
    }

    #[test]
    fn anything_else_is_unknown() {
        let t = t0();
        for p in ["notes.txt", "G/my_edit.WAD", "savestates/a.p2s", "Cargo.toml"] { assert_eq!(classify(p, &t), Class::Unknown, "{p}"); }
    }

    #[test]
    fn the_committed_table_loads() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/rc-extract/data");
        let t = load_tier0(&dir).unwrap();
        assert!(t.contains("RC2.HDR") && t.contains("SCUS_972.68"), "{}", t.len());
    }
}
