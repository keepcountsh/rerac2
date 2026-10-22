//! The archive plan: which files to write, from which bytes of the image, with which expected hash. Going Commando
//! keeps its assets in the ISO filesystem (`RC2.HDR` and `G/*.WAD`), so the plan is the filesystem's files as they
//! are.

use crate::build_db::TableEntry;
use crate::{Code, Error};
use rc_formats::iso9660::IsoImage;
use std::collections::HashMap;
use std::io::{Read, Seek};

/// One file to copy (or hash).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Job {
    /// Archive-relative, `/`-separated (`global/mpegs/073.bin`).
    pub path: String,
    /// Byte offset in the image (2048-byte sectors, so = file offset).
    pub offset: u64,
    pub bytes: u64,
    /// Expected SHA-1 from the build's table.
    pub sha1: Option<[u8; 20]>,
}

/// Every file of the ISO filesystem by its disc path (`G/LEVEL0.WAD`), except `G/DUMMY.`, the
/// padding file at the end of the disc (a copy of `G/MISC.WAD`'s size that nothing opens).
pub fn iso_jobs<R: Read + Seek>(iso: &IsoImage<R>) -> Vec<Job> {
    iso.entries().iter()
        .filter(|e| !e.is_directory && e.path != "/G/DUMMY.")
        .map(|e| Job { path: e.path.trim_start_matches('/').to_string(), offset: e.lba as u64 * 2048, bytes: e.size as u64, sha1: None })
        .collect()
}

/// Attaches the table's hashes to the disc plan. The plan comes from the disc's own ToC, so a disagreement in the
/// file list or a size means a damaged image (code 40).
pub fn with_table(jobs: Vec<Job>, table: &[TableEntry]) -> Result<Vec<Job>, Error> {
    let by_path: HashMap<&str, &TableEntry> = table.iter().map(|e| (e.path.as_str(), e)).collect();
    let mut problems = Vec::new();
    let mut out = Vec::with_capacity(jobs.len());
    for mut j in jobs {
        match by_path.get(j.path.as_str()) {
            None => problems.push(format!("{} is not in the build table", j.path)),
            Some(e) if e.size != j.bytes => problems.push(format!("{}: {} bytes on this disc, {} expected", j.path, j.bytes, e.size)),
            Some(e) => j.sha1 = Some(e.sha1),
        }
        out.push(j);
    }
    let planned: std::collections::HashSet<&str> = out.iter().map(|j| j.path.as_str()).collect();
    problems.extend(table.iter().filter(|e| !planned.contains(e.path.as_str())).map(|e| format!("{} is missing from this disc's table of contents", e.path)));
    if !problems.is_empty() {
        let n = problems.len();
        problems.truncate(5);
        return Err(Error::new(Code::VerifyFailed, format!(
            "the disc's table of contents does not match this build ({n} differences: {}{}); the image is damaged",
            problems.join("; "), if n > 5 { "; …" } else { "" })));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_join() {
        let job = |p: &str, n: u64| Job { path: p.into(), offset: 0, bytes: n, sha1: None };
        let entry = |p: &str, n: u64| TableEntry { path: p.into(), size: n, sha1: [n as u8; 20] };
        let table = [entry("RC2.HDR", 3), entry("G/LEVEL0.WAD", 4)];
        let jobs = with_table(vec![job("RC2.HDR", 3), job("G/LEVEL0.WAD", 4)], &table).unwrap();
        assert_eq!(jobs[1].sha1, Some([4; 20]));
        let e = with_table(vec![job("RC2.HDR", 5)], &table).unwrap_err();
        assert_eq!(e.code, Code::VerifyFailed);
        assert!(e.message.contains("2 differences"), "{}", e.message);
    }
}
