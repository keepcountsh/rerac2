//! `class-census`: the static census of the unported moby classes (docs/plan/class_census.md).
//!
//! For every level 00–18 it reads the overlay (`extracted/levels/NN/overlay.bin`), the class table (`lvl.vtbl`), the
//! port's class registry ([`LevelPorts`]) and the placed moby instances (placed = in the gameplay instance list,
//! created = the loader's spawn test on a first visit, as `all_levels_smoke` counts them). It builds the level's
//! static call graph and, from the update function of every class the port does not run, the class's **unit**: the
//! update plus every function only it reaches (a callee joins the unit when all its callers are in the unit). The
//! callees left outside are the unit's **shared functions** (boot ELF, engine / level code other code also calls).
//!
//! Edges: `jal` (call), `j` out of the function (tail), a code address formed with `lui`/`%lo` (ptr: a callback or
//! a state function handed on), a `.data` table of function starts formed the same way (table), and the fall-through
//! into the next start when a function does not end in `jr ra` / `j` (fall). `jalr` sites in the unit are counted as
//! unresolved indirect calls.
//!
//! **Direct reads of engine state** (data): a unit's own code that forms a data address with `lui`/`%lo` (the boot
//! ELF's globals below the overlay base, or overlay data) depends on that global. Only the globals the tag table
//! names (`G:<address>` rows: the state a system owns, e.g. the ship moby `0x140940` of the ship-combat mode) count
//! as a dependency; every boot global an unported unit touches is listed in `globals.tsv` for the next tagging pass.
//!
//! Identity across levels uses the port's code identity ([`rc_formats::level_overlay::mask`], the `Relocation`
//! masking): a shared function is keyed by its level-01 copy (`L01:addr`), its boot address (`boot:addr`), or, when
//! level 01 has no copy, by the hash of its masked code (`H:hash`, with a representative level address). Units are
//! grouped across levels by the hash of their update's masked code (one class-port unit, as `LevelPorts` groups).
//!
//! Outputs (tab-separated, `work/census/` by default): `classes.tsv`, `units.tsv`, `shared.tsv`, `globals.tsv`, and,
//! when a system tag table is given (`tools/ghidra/names/census_systems.tsv`), `systems.tsv` and `unit_systems.tsv`.

use anyhow::{Context, Result};
use rc_formats::level_overlay::{address_refs_gp, mask, LevelOverlay, Relocation, TEXT_SECTION};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The boot ELF's load address: a `lui`/`%lo` constant below it is not an address.
const BOOT_BASE: u32 = 0x10_0000;
const JR_RA: u32 = 0x03e0_0008;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Edge {
    Call,
    Tail,
    Ptr,
    Table,
    Fall,
    /// A data address the unit's own code forms (a global it reads or writes), not a call.
    Data,
}

impl Edge {
    fn tag(self) -> &'static str {
        match self {
            Edge::Call => "call",
            Edge::Tail => "tail",
            Edge::Ptr => "ptr",
            Edge::Table => "table",
            Edge::Fall => "fall",
            Edge::Data => "data",
        }
    }
}

/// A level's static call graph.
pub struct Graph {
    pub starts: BTreeSet<u32>,
    /// Function → (callee, edge kind).
    pub callees: HashMap<u32, BTreeSet<(u32, Edge)>>,
    pub callers: HashMap<u32, BTreeSet<u32>>,
    /// Function → `jalr` sites (not `jr ra`).
    pub jalr: HashMap<u32, usize>,
    pub extent: HashMap<u32, usize>,
    /// Function → the data keys it forms: `G:<address>` for a global it reads or writes (outside `.text`, at or
    /// above the boot ELF's 0x100000), `G:<address>=<value>` for a loaded global compared with a constant.
    pub data: HashMap<u32, BTreeSet<String>>,
    /// The starts with evidence of their own (a `jal` target, a class-table update, or right after a `jr ra` +
    /// delay slot), as opposed to a code address only formed with `lui`/`%lo`.
    pub strong: BTreeSet<u32>,
    /// The overlay's first section (below it: the boot ELF).
    pub base: u32,
}

fn words_of(s: &rc_formats::font::OverlaySection) -> Vec<u32> { s.data.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect() }

/// The function starts exactly as `LevelOverlay` finds them (its set is private; same rules), the `lui`/`%lo` and
/// `$gp` addresses with the game's `$gp`.
fn starts_of(ov: &LevelOverlay, gp: u32) -> (u32, u32, BTreeSet<u32>) {
    let Some(t) = ov.sections.get(TEXT_SECTION) else { return (0, 0, BTreeSet::new()) };
    let text = words_of(t);
    let (ts, te) = (t.dest, t.dest + 4 * text.len() as u32);
    let mut s: BTreeSet<u32> = text.iter().filter(|&&w| w >> 26 == 3).map(|&w| (w & 0x03ff_ffff) << 2).filter(|&a| a >= ts && a < te).collect();
    s.extend(ov.vtbl().iter().map(|e| e.update).filter(|&a| a >= ts && a < te));
    for p in 2..text.len() {
        let w = text[p];
        if text[p - 2] == JR_RA && w >> 16 == 0x27bd && w & 0x8000 != 0 { s.insert(ts + 4 * p as u32); }
    }
    s.extend(address_refs_gp(&text, gp).into_iter().map(|(_, a)| a).filter(|&a| a >= ts && a < te && a % 4 == 0));
    (ts, te, s)
}

impl Graph {
    /// R&C1's graph: its overlay base and `$gp`.

    /// The graph of an overlay based at `base` with the game's `$gp` (Going Commando: 0x1a7c80, 0x1aeff0).
    pub fn build_with(ov: &LevelOverlay, base: u32, gp: u32) -> Graph {
        let (ts, te, starts) = starts_of(ov, gp);
        // A code address formed with `lui`/`%lo` is taken as a function only with other evidence of a start (a
        // `jal` target, a class-table update, or right after a `jr ra` + delay slot): constants such as the colour
        // 0x202020 also look like `.text` addresses.
        let text = ov.sections.get(TEXT_SECTION).map(words_of).unwrap_or_default();
        let at = |a: u32| text.get(((a - ts) / 4) as usize).copied().unwrap_or(0);
        let mut strong: BTreeSet<u32> = text.iter().filter(|&&w| w >> 26 == 3).map(|&w| (w & 0x03ff_ffff) << 2).collect();
        strong.extend(ov.vtbl().iter().map(|e| e.update));
        let is_fn = |a: u32| strong.contains(&a) || (a >= ts + 8 && at(a - 8) == JR_RA);
        let strong_starts: BTreeSet<u32> = starts.iter().copied().filter(|&a| is_fn(a)).collect();
        let mut g = Graph { starts: starts.clone(), callees: HashMap::new(), callers: HashMap::new(), jalr: HashMap::new(), extent: HashMap::new(), data: HashMap::new(), strong: strong_starts, base };
        for &f in &starts {
            let Some(n) = ov.extent(f) else { continue };
            let Some(code) = ov.code(f, n) else { continue };
            g.extent.insert(f, n);
            let end = f + 4 * n as u32;
            let mut out: BTreeSet<(u32, Edge)> = BTreeSet::new();
            let mut jalr = 0;
            for &w in code {
                match w >> 26 {
                    3 => { out.insert(((w & 0x03ff_ffff) << 2, Edge::Call)); }
                    2 => {
                        let a = (w & 0x03ff_ffff) << 2;
                        if a < f || a >= end { out.insert((a, Edge::Tail)); }
                    }
                    0 if w & 63 == 9 => jalr += 1,
                    _ => {}
                }
            }
            let mut data: BTreeSet<String> = data_refs(code, ts, te);
            for (_, a) in address_refs_gp(code, gp) {
                if a >= ts && a < te {
                    if (a < f || a >= end) && starts.contains(&a) && is_fn(a) { out.insert((a, Edge::Ptr)); }
                    continue;
                }
                if a >= BOOT_BASE { data.insert(format!("G:{a:08x}")); }
                if a >= base && a % 4 == 0 {
                    // A table of function starts (entries inside this function are a switch's jump table).
                    let mut p = a;
                    for _ in 0..256 {
                        let Some(v) = ov.u32(p) else { break };
                        if !(v >= ts && v < te && starts.contains(&v) && is_fn(v)) { break; }
                        if v < f || v >= end { out.insert((v, Edge::Table)); }
                        p += 4;
                    }
                }
            }
            // Fall-through: the extent (trailing padding `nop`s dropped) does not end in `jr ra`, `j` or `b` + delay
            // slot.
            let m = code.iter().rposition(|&w| w != 0).map_or(0, |i| i + 1);
            let terminal = |w: u32| w == JR_RA || w >> 26 == 2 || w >> 16 == 0x1000;
            // Only into a start that has no evidence of its own (a code address formed with `lui`/`%lo` inside a
            // function): a real function start is never fallen into (a loop's backward branch can end a function).
            if m >= 1 && !terminal(code[m - 1]) && !(m >= 2 && terminal(code[m - 2])) && end < te && starts.contains(&end) && !is_fn(end) {
                out.insert((end, Edge::Fall));
            }
            if jalr > 0 { g.jalr.insert(f, jalr); }
            if !data.is_empty() { g.data.insert(f, data); }
            for &(c, _) in &out { g.callers.entry(c).or_default().insert(f); }
            g.callees.insert(f, out);
        }
        g
    }

    /// The unit of `root`: the functions only it reaches, and the callees left outside (with their edge kinds).
    pub fn unit(&self, root: u32) -> (BTreeSet<u32>, BTreeMap<u32, BTreeSet<Edge>>) {
        let mut u: BTreeSet<u32> = BTreeSet::from([root]);
        loop {
            let mut added = false;
            let cands: Vec<u32> = u.iter().flat_map(|f| self.callees.get(f).into_iter().flatten().map(|&(c, _)| c)).filter(|c| !u.contains(c) && *c >= self.base).collect();
            for c in cands {
                if u.contains(&c) { continue; }
                let callers = self.callers.get(&c);
                if callers.is_some_and(|cs| cs.iter().all(|x| u.contains(x) || *x == c)) {
                    u.insert(c);
                    added = true;
                }
            }
            if !added { break; }
        }
        let mut leaves: BTreeMap<u32, BTreeSet<Edge>> = BTreeMap::new();
        for f in &u {
            for &(c, e) in self.callees.get(f).into_iter().flatten() {
                if !u.contains(&c) { leaves.entry(c).or_default().insert(e); }
            }
        }
        (u, leaves)
    }
}

/// The data keys `code` forms (`Graph::data`): a `lui` (+ `addiu` / `ori`) constant and every load / store whose
/// base register holds such a constant (`base + offset`: a field of a global block, e.g. the hero state `0x1413d4`
/// read as `0x13f350 + 0x2084`), and a loaded global compared with a constant (`xori` with an immediate, or a
/// `beq` / `bne` against a register holding one: the hero state against 0x32, `G:001413d4=00000032`). A linear pass
/// with one register file; a call clears the caller-saved registers, a branch is not followed (a register set on one
/// arm and read after the join is a rare extra reference, never a missed one on straight code). Addresses inside
/// `.text` (`ts..te`) and below the boot ELF are dropped.
fn data_refs(code: &[u32], ts: u32, te: u32) -> BTreeSet<String> {
    #[derive(Clone, Copy)]
    enum R {
        Const(u32),
        Load(u32),
    }
    let mut regs: [Option<R>; 32] = [None; 32];
    let mut out = BTreeSet::new();
    let ok = |a: u32| a >= BOOT_BASE && !(a >= ts && a < te);
    let mut call_pending = false;
    for &w in code {
        let (op, rs, rt, rd) = (w >> 26, (w >> 21 & 31) as usize, (w >> 16 & 31) as usize, (w >> 11 & 31) as usize);
        let imm = (w & 0xffff) as u16 as i16 as i32;
        let was_call = call_pending;
        call_pending = false;
        regs[0] = Some(R::Const(0));
        let cst = |r: Option<R>| if let Some(R::Const(c)) = r { Some(c) } else { None };
        let ld = |r: Option<R>| if let Some(R::Load(a)) = r { Some(a) } else { None };
        match op {
            0x0f => regs[rt] = Some(R::Const((w & 0xffff) << 16)),
            0x09 | 0x19 => regs[rt] = cst(regs[rs]).map(|b| R::Const(b.wrapping_add(imm as u32))),
            0x0d => regs[rt] = cst(regs[rs]).map(|b| R::Const(b | (w & 0xffff))),
            0x0e => {
                if let Some(a) = ld(regs[rs]) { out.insert(format!("G:{a:08x}={:08x}", w & 0xffff)); }
                regs[rt] = None;
            }
            0x04 | 0x05 => {
                if let (Some(a), Some(c)) = (ld(regs[rs]), cst(regs[rt])) { out.insert(format!("G:{a:08x}={c:08x}")); }
                if let (Some(a), Some(c)) = (ld(regs[rt]), cst(regs[rs])) { out.insert(format!("G:{a:08x}={c:08x}")); }
            }
            // Loads and stores: lb..lwu, lq / sq, lwc1 / swc1, ldc2 / sdc2 (lqc2 / sqc2), ld / sd.
            0x20..=0x2e | 0x1e | 0x1f | 0x31 | 0x39 | 0x36 | 0x3e | 0x37 | 0x3f => {
                let a = cst(regs[rs]).map(|b| b.wrapping_add(imm as u32)).filter(|&a| ok(a));
                if let Some(a) = a { out.insert(format!("G:{a:08x}")); }
                if matches!(op, 0x20..=0x27 | 0x1e | 0x37) { regs[rt] = a.map(R::Load); }
            }
            0x03 => { regs[31] = None; call_pending = true; }
            0x00 => {
                if w & 63 == 9 { regs[rd] = None; call_pending = true; } else if w & 63 != 8 { regs[rd] = None; }
            }
            0x1c => regs[rd] = None,
            0x11 | 0x12 => { if rs < 4 { regs[rt] = None; } }
            0x08 | 0x0a..=0x0c | 0x18 => regs[rt] = None,
            _ => {}
        }
        if was_call {
            // The delay slot has run: the callee clobbers everything but s0..s7, gp, sp, fp.
            for r in (1..16).chain(24..28).chain([31]) { regs[r] = None; }
        }
    }
    out
}

fn hash_words(w: &[u32]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    w.hash(&mut h);
    h.finish()
}

/// Masked-code hash of the function at `f` over its extent.
fn code_hash(ov: &LevelOverlay, f: u32) -> Option<u64> { Some(hash_words(&mask(ov.code(f, ov.extent(f)?)?))) }

/// The reference level's functions (8 words or more) by masked-code hash, and each one's hash. Functions that differ
/// only in the addresses they form (the draw-list registrars `0x2e7718` / `0x2e7920`) share a hash: such a hash names
/// no one function (`None`).
#[derive(Default)]
struct RefHashes {
    at: HashMap<u32, u64>,
    by: HashMap<u64, Option<u32>>,
}

impl RefHashes {
    fn build(ov: &LevelOverlay, graph: &Graph) -> RefHashes {
        let mut r = RefHashes::default();
        for &f in &graph.starts {
            if !graph.extent.get(&f).is_some_and(|&n| n >= 8) { continue; }
            let Some(h) = code_hash(ov, f) else { continue };
            r.at.insert(f, h);
            r.by.entry(h).and_modify(|a| if *a != Some(f) { *a = None }).or_insert(Some(f));
        }
        r
    }
}

/// A shared function's key: its boot address (`boot:`), else its reference-level copy (`L00:`): the function at the same
/// address with the same masked code (engine code sits at the same addresses on every level), the relocation's, or
/// `refs.by` (a copy `Relocation` misses: the same code over the same extent, linked elsewhere, when the hash names one
/// function); else the masked-code hash (`H:`).
#[allow(clippy::too_many_arguments)]
fn key_in(g: &Layout, level: u32, f: u32, ov: &LevelOverlay, to_ref: &Relocation, reference: &Graph, refs: &RefHashes) -> (String, bool) {
    let r = g.ref_level;
    if f < g.base { return (format!("boot:{f:08x}"), true); }
    if level == r { return (format!("L{r:02}:{f:08x}"), true); }
    if refs.at.get(&f).is_some_and(|&h| code_hash(ov, f) == Some(h)) { return (format!("L{r:02}:{f:08x}"), true); }
    let n = ov.extent(f).unwrap_or(0);
    if let Some(a) = to_ref.func(f).filter(|a| reference.starts.contains(a) && (n >= 8 || reference.extent.get(a) == Some(&n))) {
        return (format!("L{r:02}:{a:08x}"), true);
    }
    let h = code_hash(ov, f).unwrap_or(0);
    if let Some(Some(a)) = refs.by.get(&h) { return (format!("L{r:02}:{a:08x}"), true); }
    (format!("H:{h:016x}"), false)
}

/// The overlays' layout and the reference level.
pub struct Layout {
    /// The overlay's first section (`.lit`, 0x1a7c80 on every level): code below it is the boot ELF's.
    pub base: u32,
    pub gp: u32,
    /// The level whose addresses name the shared functions (`L00:`, Aranos: the level with a decompiler export).
    pub ref_level: u32,
    /// The decompiler exports of the reference level and the boot ELF (`work/decomp/<program>`).
    pub ref_program: &'static str,
    pub boot_program: &'static str,
    /// Where the port's citations count (`repo_refs`): path prefixes under the repo.
    pub cites: &'static [&'static str],
}

pub const LAYOUT: Layout = Layout { base: 0x1a_7c80, gp: 0x1a_eff0, ref_level: 0, ref_program: "level00.elf", boot_program: "SCUS_972.68", cites: &["crates/", "docs/"] };

/// Function names from the decompiler export (`work/decomp/<program>/index.tsv`).
pub fn names(work: &Path, program: &str) -> HashMap<u32, String> {
    let Ok(s) = std::fs::read_to_string(work.join("decomp").join(program).join("index.tsv")) else { return HashMap::new() };
    s.lines().filter_map(|l| { let mut it = l.split('\t'); Some((u32::from_str_radix(it.next()?, 16).ok()?, it.next()?.to_string())) }).collect()
}

/// Hex tokens cited in the repo: token value → (files under crates/, files under docs/). Lower-case `0x…` (5–8 digits)
/// and `fun_XXXXXXXX`.
fn repo_refs(root: &Path, cites: &[&str]) -> HashMap<u32, (BTreeSet<String>, BTreeSet<String>)> {
    let mut out: HashMap<u32, (BTreeSet<String>, BTreeSet<String>)> = HashMap::new();
    fn walk(d: &Path, ext: &str, files: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() { walk(&p, ext, files); } else if p.extension().is_some_and(|x| x == ext) { files.push(p); }
        }
    }
    for (dir, ext, is_doc) in [("crates", "rs", false), ("docs", "md", true)] {
        let mut files = Vec::new();
        walk(&root.join(dir), ext, &mut files);
        for p in files {
            let Ok(s) = std::fs::read_to_string(&p) else { continue };
            let s = s.to_ascii_lowercase();
            let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().into_owned();
            if !cites.iter().any(|c| rel.starts_with(c)) { continue; }
            let b = s.as_bytes();
            let mut i = 0;
            while i + 2 < b.len() {
                let (skip, start) = if b[i] == b'0' && b[i + 1] == b'x' { (2, true) } else if b[i..].starts_with(b"fun_") { (4, true) } else { (0, false) };
                if start && (i == 0 || !b[i - 1].is_ascii_alphanumeric()) {
                    let j0 = i + skip;
                    let mut j = j0;
                    while j < b.len() && b[j].is_ascii_hexdigit() { j += 1; }
                    let n = j - j0;
                    if (5..=8).contains(&n) && (j == b.len() || !b[j].is_ascii_alphanumeric()) {
                        if let Ok(v) = u32::from_str_radix(&s[j0..j], 16) {
                            let e = out.entry(v).or_default();
                            if is_doc { e.1.insert(rel.clone()); } else { e.0.insert(rel.clone()); }
                        }
                    }
                    i = j.max(i + 1);
                    continue;
                }
                i += 1;
            }
        }
    }
    out
}

/// One class on one level.
struct ClassRow {
    level: u32,
    o_class: i32,
    placed: usize,
    created: usize,
    update: u32,
    port: String,
    unit: usize,
}

#[derive(Default)]
struct Unit {
    /// (level, update address in that level).
    copies: BTreeSet<(u32, u32)>,
    l01: Option<u32>,
    classes: BTreeSet<(u32, i32)>,
    placed: usize,
    created: usize,
    private_fns: usize,
    private_words: usize,
    jalr: usize,
    /// Shared key → edge kinds.
    leaves: BTreeMap<String, BTreeSet<Edge>>,
    ported: bool,
}

#[derive(Default)]
struct Shared {
    /// (level, address) of every copy seen.
    sites: BTreeSet<(u32, u32)>,
    units: BTreeSet<usize>,
    in_ported_unit: bool,
    /// Direct callees (keys) of the representative copy.
    callees: BTreeSet<String>,
    words: usize,
}

/// A tag row of `census_systems.tsv`.
#[derive(Clone, Debug, Default)]
struct Tag {
    name: String,
    system: String,
    status: String,
    conf: String,
}

/// The tag table: key → tag. Keys: `L01:` / `boot:` / `C:` / `H:` functions, `G:<address>` globals.
fn tags(path: &Path) -> HashMap<String, Tag> {
    let Ok(s) = std::fs::read_to_string(path) else { return HashMap::new() };
    s.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            if c.len() < 4 || c[0] == "key" { return None; }
            Some((c[0].to_string(), Tag { name: c[1].to_string(), system: c[2].to_string(), status: c[3].to_string(), conf: c.get(4).unwrap_or(&"").to_string() }))
        })
        .collect()
}

/// A system's tally: (units, classes as (level, class), created instances, blocking function → units, status).
type SystemTally = (BTreeSet<usize>, BTreeSet<(u32, i32)>, usize, BTreeMap<String, usize>, String);

pub struct Summary {
    pub classes: usize,
    pub placed: usize,
    pub created: usize,
    pub units: usize,
    pub shared: usize,
    pub out: PathBuf,
}

/// One level's inputs: its overlay, its classes' placed and created instances, and the port's name for each class
/// it runs.
pub struct LevelIn {
    pub ov: Arc<LevelOverlay>,
    pub placed: BTreeMap<i32, (usize, usize)>,
    pub ports: HashMap<i32, String>,
}

/// The census: every `G/LEVELn.WAD` of `extracted` (the data folder),
/// its overlay (the data lump 0), its placed moby instances and the static spawn's test on a first visit with an empty
/// save (`rc_game::moby::SpawnRules`), and rc-game's class registry (`moby::classes::update_for`) as the port.
pub fn run(extracted: &Path, repo: &Path, work: &Path, out: &Path, tag_file: Option<&Path>) -> Result<Summary> {
    use rc_formats::rac2::level::{LevelWad, Lump, Part};
    let mut inputs = Vec::new();
    for level in 0..40u32 {
        let Ok(file) = std::fs::read(extracted.join(format!("G/LEVEL{level}.WAD"))) else { continue };
        let wad = LevelWad::parse(&file).with_context(|| format!("LEVEL{level}.WAD"))?;
        let Some(ob) = wad.lump(Lump::Overlay)? else { eprintln!("level {level:02}: no overlay"); continue };
        let ov = Arc::new(LevelOverlay::parse(ob).with_context(|| format!("level {level:02} overlay"))?);
        let g = wad.part_bytes(Part::Gameplay)?.with_context(|| format!("level {level:02} gameplay"))?;
        let g = if rc_formats::wad::is_wad(g) { rc_formats::wad::decompress(g)? } else { g.to_vec() };
        let records = rc_formats::rac2::gameplay::moby_instances(&g).with_context(|| format!("level {level:02} instances"))?;
        let mut game = rc_game::game::GameState::default();
        let ground = |_: [f32; 3]| None;
        let mut rules = rc_game::moby::SpawnRules { game: &mut game, fresh: true, ground: &ground };
        let mut placed: BTreeMap<i32, (usize, usize)> = BTreeMap::new();
        for r in &records {
            let c = r.base.o_class;
            if c == 0 { continue; }
            let skip = rules.test(r).0;
            let e = placed.entry(c).or_default();
            e.0 += 1;
            if !skip { e.1 += 1; }
        }
        let ports: HashMap<i32, String> = ov.vtbl().iter().filter(|e| rc_game::moby::classes::update_for(e.o_class as i16).is_some()).map(|e| (e.o_class, "rc_game".to_string())).collect();
        inputs.push((level, LevelIn { ov, placed, ports }));
    }
    census(&LAYOUT, inputs, repo, work, out, tag_file)
}

/// The census over the levels' inputs (module docs).
fn census(g: &Layout, inputs: Vec<(u32, LevelIn)>, repo: &Path, work: &Path, out: &Path, tag_file: Option<&Path>) -> Result<Summary> {
    let l01 = inputs.iter().find(|(l, _)| *l == g.ref_level).map(|(_, i)| i.ov.clone()).with_context(|| format!("level {:02} overlay", g.ref_level))?;
    let l01_names = names(work, g.ref_program);
    let boot_names = names(work, g.boot_program);
    let l01_graph = Graph::build_with(&l01, g.base, g.gp);
    let by_hash = RefHashes::build(&l01, &l01_graph);
    let tagmap = tag_file.map(tags).unwrap_or_default();
    let rp = format!("L{:02}:", g.ref_level);

    let mut rows: Vec<ClassRow> = Vec::new();
    let mut units: Vec<Unit> = Vec::new();
    let mut unit_by_hash: HashMap<(u64, bool), usize> = HashMap::new();
    let mut shared: BTreeMap<String, Shared> = BTreeMap::new();
    // Boot global key → (units whose own code forms it, one is ported).
    let mut globals: BTreeMap<String, (BTreeSet<usize>, bool)> = BTreeMap::new();
    let mut level_names: HashMap<u32, HashMap<u32, String>> = HashMap::new();

    for (level, input) in &inputs {
        let (level, ov, placed) = (*level, &input.ov, &input.placed);
        let graph = Graph::build_with(ov, g.base, g.gp);
        let to_l01 = Relocation::new(ov, &l01);
        let lnames = names(work, &g.ref_program.replace(&format!("level{:02}", g.ref_level), &format!("level{level:02}")));
        let mut update_classes: BTreeMap<u32, Vec<i32>> = BTreeMap::new();
        for e in ov.vtbl() {
            if e.update == 0 || e.o_class == 0 { continue; }
            update_classes.entry(e.update).or_default().push(e.o_class);
        }
        for (&update, cls) in &update_classes {
            let ported: Vec<String> = cls.iter().filter_map(|c| input.ports.get(c).cloned()).collect();
            let is_ported = !ported.is_empty();
            let (p, _c): (usize, usize) = cls.iter().map(|c| placed.get(c).copied().unwrap_or_default()).fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
            if p == 0 && !is_ported { continue; }
            let (private, leaves) = graph.unit(update);
            let h = code_hash(ov, update).unwrap_or(0);
            let uid = *unit_by_hash.entry((h, is_ported)).or_insert_with(|| {
                units.push(Unit { ported: is_ported, ..Default::default() });
                units.len() - 1
            });
            let u = &mut units[uid];
            if u.copies.is_empty() {
                u.private_fns = private.len();
                u.private_words = private.iter().map(|f| graph.extent.get(f).copied().unwrap_or(0)).sum();
                u.jalr = private.iter().map(|f| graph.jalr.get(f).copied().unwrap_or(0)).sum();
            }
            u.copies.insert((level, update));
            if u.l01.is_none() { u.l01 = if level == 1 { Some(update) } else { to_l01.func(update) }; }
            for &c in cls {
                let (pl, cr) = placed.get(&c).copied().unwrap_or_default();
                u.classes.insert((level, c));
                u.placed += pl;
                u.created += cr;
                rows.push(ClassRow { level, o_class: c, placed: pl, created: cr, update, port: ported.first().cloned().unwrap_or_default(), unit: uid });
            }
            for (&leaf, kinds) in &leaves {
                let (key, _) = key_in(g, level, leaf, ov, &to_l01, &l01_graph, &by_hash);
                u.leaves.entry(key.clone()).or_default().extend(kinds.iter().copied());
                let s = shared.entry(key.clone()).or_default();
                s.sites.insert((level, leaf));
                s.units.insert(uid);
                if is_ported { s.in_ported_unit = true; }
                if s.callees.is_empty() && leaf >= g.base {
                    s.words = graph.extent.get(&leaf).copied().unwrap_or(0);
                    for &(c, _) in graph.callees.get(&leaf).into_iter().flatten() {
                        s.callees.insert(key_in(g, level, c, ov, &to_l01, &l01_graph, &by_hash).0);
                    }
                }
            }
            // The globals the unit's own code forms: a tagged one (`G:`) is a dependency like a call. A class
            // override (`X:<class>`: state the census cannot see, with its reason in the table) counts the same way.
            let mut data: BTreeSet<String> = private.iter().flat_map(|f| graph.data.get(f).into_iter().flatten().cloned()).collect();
            data.extend(cls.iter().map(|c| format!("X:{c}")).filter(|k| tagmap.contains_key(k)));
            for key in &data {
                let a = key.get(2..10).and_then(|h| u32::from_str_radix(h, 16).ok()).unwrap_or(0);
                if key.starts_with("G:") && a < g.base {
                    let e = globals.entry(key.clone()).or_default();
                    e.0.insert(uid);
                    e.1 |= is_ported;
                }
                if !tagmap.contains_key(key) { continue; }
                u.leaves.entry(key.clone()).or_default().insert(Edge::Data);
                let s = shared.entry(key.clone()).or_default();
                s.sites.insert((level, a));
                s.units.insert(uid);
                if is_ported { s.in_ported_unit = true; }
            }
            if is_ported {
                // The functions a ported unit owns count as ported too (by their level-01 key).
                for &f in &private {
                    let (key, _) = key_in(g, level, f, ov, &to_l01, &l01_graph, &by_hash);
                    shared.entry(key).or_default().in_ported_unit = true;
                }
            }
        }
        level_names.insert(level, lnames);
    }

    let refs = repo_refs(repo, g.cites);
    let name_of = |key: &str, s: Option<&Shared>| -> String {
        if key.starts_with("G:") || key.starts_with("X:") { return tagmap.get(key).map(|t| t.name.clone()).unwrap_or_default(); }
        if let Some(a) = key.strip_prefix(rp.as_str()).and_then(|h| u32::from_str_radix(h, 16).ok()) {
            return l01_names.get(&a).cloned().unwrap_or_default();
        }
        if let Some(a) = key.strip_prefix("boot:").and_then(|h| u32::from_str_radix(h, 16).ok()) {
            return boot_names.get(&a).cloned().unwrap_or_default();
        }
        s.and_then(|s| s.sites.iter().find_map(|(l, a)| level_names.get(l).and_then(|m| m.get(a)).cloned())).unwrap_or_default()
    };
    let addr_of = |key: &str| -> Option<u32> { key.strip_prefix(rp.as_str()).or_else(|| key.strip_prefix("boot:")).or_else(|| key.strip_prefix("G:").map(|h| &h[..8.min(h.len())])).and_then(|h| u32::from_str_radix(h, 16).ok()) };

    std::fs::create_dir_all(out)?;
    // classes.tsv
    let mut t = String::from("level\to_class\tplaced\tcreated\tupdate\tunit\tport\n");
    for r in &rows { t += &format!("{:02}\t{}\t{}\t{}\t{:08x}\tU{}\t{}\n", r.level, r.o_class, r.placed, r.created, r.update, r.unit, r.port); }
    crate::write_output(&out.join("classes.tsv"), t)?;
    // units.tsv (unported only, by created instances)
    let mut order: Vec<usize> = (0..units.len()).filter(|&i| !units[i].ported).collect();
    order.sort_by_key(|&i| (std::cmp::Reverse(units[i].created), std::cmp::Reverse(units[i].placed), i));
    let mut t = String::from("unit\tcreated\tplaced\tn_classes\tlevels\tclasses\tupdates\tl01_copy\tprivate_fns\tprivate_words\tjalr\tn_shared\tshared\n");
    for &i in &order {
        let u = &units[i];
        let levels: BTreeSet<u32> = u.copies.iter().map(|c| c.0).collect();
        let classes: BTreeSet<i32> = u.classes.iter().map(|c| c.1).collect();
        t += &format!(
            "U{i}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            u.created,
            u.placed,
            classes.len(),
            levels.iter().map(|l| format!("{l:02}")).collect::<Vec<_>>().join(","),
            classes.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(","),
            u.copies.iter().map(|(l, a)| format!("L{l:02}:{a:06x}")).collect::<Vec<_>>().join(","),
            u.l01.map(|a| format!("{a:06x}")).unwrap_or_default(),
            u.private_fns,
            u.private_words,
            u.jalr,
            u.leaves.len(),
            u.leaves.iter().map(|(k, e)| format!("{k}{}", if e.contains(&Edge::Call) || e.contains(&Edge::Tail) { String::new() } else { format!("[{}]", e.iter().map(|x| x.tag()).collect::<Vec<_>>().join("+")) })).collect::<Vec<_>>().join(";")
        );
    }
    crate::write_output(&out.join("units.tsv"), t)?;
    // shared.tsv (functions unported units call)
    let mut srow: Vec<(&String, &Shared, usize, usize)> = shared
        .iter()
        .filter(|(_, s)| s.units.iter().any(|&u| !units[u].ported))
        .map(|(k, s)| {
            let un: Vec<usize> = s.units.iter().copied().filter(|&u| !units[u].ported).collect();
            (k, s, un.len(), un.iter().map(|&u| units[u].created).sum::<usize>())
        })
        .collect();
    srow.sort_by_key(|r| (std::cmp::Reverse(r.3), std::cmp::Reverse(r.2), r.0.clone()));
    // An `H:` function's nearest reference function (same length within 4 words, at least 85 % of the masked words
    // equal): for reading it in the reference level's decompiler export.
    let ref_masked: Vec<(u32, Vec<u32>)> = l01_graph.starts.iter().filter_map(|&f| Some((f, mask(l01.code(f, *l01_graph.extent.get(&f)?)?)))).collect();
    let near = |k: &str, sh: &Shared| -> String {
        if !k.starts_with("H:") { return String::new(); }
        let Some(&(lv, a)) = sh.sites.iter().next() else { return String::new() };
        let Some(ov) = inputs.iter().find(|(l, _)| *l == lv).map(|(_, i)| &i.ov) else { return String::new() };
        let Some(m) = ov.extent(a).and_then(|n| ov.code(a, n)).map(mask) else { return String::new() };
        let mut best = (0.0f32, 0u32);
        for (f, w) in &ref_masked {
            if w.len().abs_diff(m.len()) > 4 || m.len() < 4 { continue; }
            let eq = w.iter().zip(&m).filter(|(x, y)| x == y).count();
            let sc = eq as f32 / w.len().max(m.len()) as f32;
            if sc > best.0 { best = (sc, *f); }
        }
        if best.0 >= 0.85 { format!("L{:02}:{:08x}@{:.2}", g.ref_level, best.1, best.0) } else { String::new() }
    };
    let mut t = String::from("key\tname\tunported_units\tcreated\tported_unit_uses\tcrates_refs\tdocs_refs\twords\trep_site\tn_levels\tsystem\tstatus\tconf\tcrates_files\tdocs_files\tcallees\tnear\n");
    for (k, s, n, c) in &srow {
        let (cr, dr) = addr_of(k).and_then(|a| refs.get(&a)).map(|(c, d)| (c.len(), d.len())).unwrap_or((0, 0));
        let files = |doc: bool| -> String {
            addr_of(k).and_then(|a| refs.get(&a)).map(|(c, d)| (if doc { d } else { c }).iter().take(4).map(|f| f.trim_start_matches("crates/").trim_start_matches("docs/plan/").to_string()).collect::<Vec<_>>().join(",")).unwrap_or_default()
        };
        let nlev = s.sites.iter().map(|x| x.0).collect::<BTreeSet<_>>().len();
        let tag = tagmap.get(*k);
        let rep = s.sites.iter().next().map(|(l, a)| format!("L{l:02}:{a:06x}")).unwrap_or_default();
        t += &format!(
            "{k}\t{}\t{n}\t{c}\t{}\t{cr}\t{dr}\t{}\t{rep}\t{nlev}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            name_of(k, Some(s)),
            s.in_ported_unit as u8,
            s.words,
            tag.map(|t| t.system.as_str()).unwrap_or(""),
            tag.map(|t| t.status.as_str()).unwrap_or(""),
            tag.map(|t| t.conf.as_str()).unwrap_or(""),
            files(false),
            files(true),
            s.callees.iter().cloned().collect::<Vec<_>>().join(";"),
            near(k, s)
        );
    }
    crate::write_output(&out.join("shared.tsv"), t)?;
    // globals.tsv (boot globals the unported units' own code forms, by the instances of those units)
    let mut grow: Vec<(&String, Vec<usize>, bool)> = globals
        .iter()
        .map(|(k, (us, p))| (k, us.iter().copied().filter(|&u| !units[u].ported).collect::<Vec<_>>(), *p))
        .filter(|r| !r.1.is_empty())
        .collect();
    grow.sort_by_key(|r| (std::cmp::Reverse(r.1.iter().map(|&u| units[u].created).sum::<usize>()), r.0.clone()));
    let mut t = String::from("global\tunported_units\tcreated\tin_ported_unit\tcrates_refs\tsystem\tstatus\tunits\n");
    for (k, us, p) in &grow {
        let tag = tagmap.get(*k);
        t += &format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            &k[2..],
            us.len(),
            us.iter().map(|&u| units[u].created).sum::<usize>(),
            *p as u8,
            addr_of(k).and_then(|a| refs.get(&a)).map(|(c, _)| c.len()).unwrap_or(0),
            tag.map(|t| t.system.as_str()).unwrap_or(""),
            tag.map(|t| t.status.as_str()).unwrap_or(""),
            us.iter().map(|u| format!("U{u}")).collect::<Vec<_>>().join(",")
        );
    }
    crate::write_output(&out.join("globals.tsv"), t)?;

    // With a tag table: per-unit systems and the system ranking.
    if !tagmap.is_empty() {
        let mut t = String::from("unit\tcreated\tclasses\tlevels\tverdict\tmissing\tpartly\thas\tfamily\tjalr\tblocking_functions\tuntagged\n");
        let mut sys: BTreeMap<String, SystemTally> = BTreeMap::new();
        for &i in &order {
            let u = &units[i];
            let mut by: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
            let mut untagged = Vec::new();
            for k in u.leaves.keys() {
                match tagmap.get(k) {
                    Some(Tag { system, status, .. }) => {
                        by.entry(status.as_str()).or_default().insert(system.clone());
                        if status == "missing" || status == "partly" {
                            let e = sys.entry(system.clone()).or_default();
                            e.0.insert(i);
                            e.1.extend(u.classes.iter().copied());
                            *e.3.entry(k.clone()).or_default() += 1;
                        }
                    }
                    None => untagged.push(k.clone()),
                }
            }
            let need = |s: &str| by.get(s).map(|v| v.iter().cloned().collect::<Vec<_>>().join(",")).unwrap_or_default();
            let levels: BTreeSet<u32> = u.copies.iter().map(|c| c.0).collect();
            let classes: BTreeSet<i32> = u.classes.iter().map(|c| c.1).collect();
            let verdict = if by.contains_key("missing") { "missing" } else if by.contains_key("partly") { "partly" } else if !untagged.is_empty() { "unknown" } else { "cheap" };
            let blocking: Vec<&String> = u.leaves.keys().filter(|k| tagmap.get(*k).is_some_and(|t| t.status == "missing" || t.status == "partly")).collect();
            t += &format!(
                "U{i}\t{}\t{}\t{}\t{verdict}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                u.created,
                classes.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(","),
                levels.iter().map(|l| format!("{l:02}")).collect::<Vec<_>>().join(","),
                need("missing"),
                need("partly"),
                need("has"),
                need("family"),
                u.jalr,
                blocking.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(";"),
                untagged.join(";")
            );
        }
        crate::write_output(&out.join("unit_systems.tsv"), t)?;
        for (k, e) in sys.iter_mut() {
            e.2 = e.0.iter().map(|&u| units[u].created).sum();
            // The system's status: partly when the port has any of its functions, else missing.
            let has = tagmap.values().any(|t| &t.system == k && t.status != "missing");
            e.4 = if has { "partly".into() } else { "missing".into() };
        }
        let mut ranked: Vec<_> = sys.iter().collect();
        ranked.sort_by_key(|(k, e)| (std::cmp::Reverse(e.2), (*k).clone()));
        let mut t = String::from("system\tstatus\tunits\tclass_levels\tcreated\texample_classes\tkey_functions\n");
        for (k, e) in ranked {
            let mut ex: Vec<(usize, usize)> = e.0.iter().map(|&u| (units[u].created, u)).collect();
            ex.sort_by(|a, b| b.cmp(a));
            let exs: Vec<String> = ex.iter().take(4).map(|&(c, u)| format!("{} ({c})", units[u].classes.iter().map(|x| x.1).collect::<BTreeSet<_>>().iter().take(3).map(|x| x.to_string()).collect::<Vec<_>>().join("/"))).collect();
            let mut fns: Vec<(&String, &usize)> = e.3.iter().collect();
            fns.sort_by(|a, b| b.1.cmp(a.1));
            t += &format!("{k}\t{}\t{}\t{}\t{}\t{}\t{}\n", e.4, e.0.len(), e.1.len(), e.2, exs.join("; "), fns.iter().take(6).map(|(f, n)| { let nm = name_of(f, shared.get(*f)); format!("{f}{}×{n}", if nm.is_empty() { String::new() } else { format!(" {nm}") }) }).collect::<Vec<_>>().join(" "));
        }
        crate::write_output(&out.join("systems.tsv"), t)?;
    }

    let un: Vec<&ClassRow> = rows.iter().filter(|r| r.port.is_empty()).collect();
    Ok(Summary {
        classes: un.len(),
        placed: un.iter().map(|r| r.placed).sum(),
        created: un.iter().map(|r| r.created).sum(),
        units: order.len(),
        shared: srow.len(),
        out: out.to_path_buf(),
    })
}

