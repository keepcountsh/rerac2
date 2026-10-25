//! `rc-trace`: development tools against the PS2's EE memory and the level overlays: the class census, savestate and
//! EE-memory reads, PINE. Doc: tools/trace/README.md, docs/workflows/pcsx2.md. Dev tool: output goes to `work/`, never
//! `extracted/`.

use anyhow::{bail, Context, Result};
use rc_trace::ee::{self, EeSource, Savestate};
use rc_trace::{pine, trace_out_dir, write_output};
use std::path::PathBuf;

const USAGE: &str = "\
rc-trace - development tools (tools/trace/README.md, docs/workflows/pcsx2.md)

Where things go: generated output defaults to work/ (RC_WORK overrides it); nothing is ever written into
extracted/. Personal material lives in ~/PS2/ratchet2/ (RC_PERSONAL overrides it): kept savestates in savestates/.

EE memory source (for the commands that read RAM), one of:
  --state <file.p2s | dir | latest | NAME>   PCSX2 savestate; 'latest' = newest SCUS-97268*.p2s in
                                      ~/Library/Application Support/PCSX2/sstates; NAME = a kept
                                      savestate ~/PS2/ratchet2/savestates/NAME.p2s
  --ee <eeMemory.bin>                 raw 32 MiB EE RAM dump
  --pine [slot]                       live read over PCSX2's PINE socket (default slot 28011)

Commands:
  class-census [--out DIR] [--tags FILE] [--extracted DIR]
      Static census of the unported moby classes on every level (docs/plan/class_census.md): per class the
      placed / created instances, its class-port unit (update + private callees, grouped across levels by code
      identity) and the shared functions it calls. Writes classes.tsv, units.tsv, shared.tsv and, with the tag
      table (default tools/ghidra/names/census_systems.tsv), unit_systems.tsv + systems.tsv to work/census/
      (--out). Also the ported units' gaps (ported.tsv, ported_systems.tsv), the mode roots' unported engine code
      (tools/census/roots.tsv in; roots.tsv, engine_uncited.tsv) and the Aranos-only lint (aranos_only.tsv).
  info --state <p2s>                  list savestate entries and version
  dump-ee <source> [--out FILE] [--entry NAME]   write EE RAM (or any savestate entry) to FILE
                                      (default work/trace/<savestate name>_ee.bin)
  find <source> --bytes HEX [--align N] [--max N]   list EE addresses holding the bytes
  read <source> --addr HEX --len N    hex-dump EE memory
  pine-info [slot]                    print PCSX2 version / game / status over PINE
  pine-savestate <state-slot> [--slot N]   ask PCSX2 to save to a state slot (like pressing F1)
  pine-write --addr HEX --bytes HEX [--slot N]   write bytes (e.g. 01ff) to EE memory over PINE (dev pokes)
";

struct Args(Vec<String>);

impl Args {
    fn flag(&mut self, name: &str) -> bool {
        match self.0.iter().position(|a| a == name) {
            Some(i) => { self.0.remove(i); true }
            None => false,
        }
    }
    fn opt(&mut self, name: &str) -> Result<Option<String>> {
        let Some(i) = self.0.iter().position(|a| a == name) else { return Ok(None) };
        if i + 1 >= self.0.len() { bail!("{name} needs a value"); }
        let v = self.0.remove(i + 1);
        self.0.remove(i);
        Ok(Some(v))
    }
    /// `--pine` with an optional numeric slot after it.
    fn pine(&mut self) -> Option<u16> {
        let i = self.0.iter().position(|a| a == "--pine")?;
        self.0.remove(i);
        if let Some(s) = self.0.get(i).and_then(|s| s.parse().ok()) { self.0.remove(i); Some(s) } else { Some(pine::DEFAULT_SLOT) }
    }
    fn source(&mut self) -> Result<EeSource> {
        if let Some(s) = self.opt("--state")? { return Ok(EeSource::State(ee::resolve_state(&s)?)); }
        if let Some(p) = self.opt("--ee")? { return Ok(EeSource::Raw(p.into())); }
        if let Some(slot) = self.pine() { return Ok(EeSource::Pine(slot)); }
        bail!("need an EE memory source: --state, --ee or --pine\n\n{USAGE}")
    }
    fn done(&self) -> Result<()> {
        if !self.0.is_empty() { bail!("unexpected arguments: {:?}\n\n{USAGE}", self.0); }
        Ok(())
    }
}

fn hex_u32(s: &str) -> Result<u32> { u32::from_str_radix(s.trim_start_matches("0x"), 16).with_context(|| format!("bad hex {s:?}")) }

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("error: {e:#}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<i32> {
    let mut a = Args(std::env::args().skip(1).collect());
    if a.0.is_empty() || a.flag("--help") || a.flag("-h") {
        print!("{USAGE}");
        return Ok(0);
    }
    let cmd = a.0.remove(0);
    match cmd.as_str() {
        "class-census" => {
            let out = a.opt("--out")?.map(PathBuf::from).unwrap_or_else(|| rc_trace::work_dir().join("census"));
            let tags = a.opt("--tags")?.map(PathBuf::from).unwrap_or_else(|| rc_trace::repo_root().join("tools/ghidra/names/census_systems.tsv"));
            let extracted = a.opt("--extracted")?.map(PathBuf::from).unwrap_or_else(rc_formats::test_data::root);
            a.done()?;
            let s = rc_trace::class_census::run(&extracted, &rc_trace::repo_root(), &rc_trace::work_dir(), &out, Some(&tags))?;
            println!(
                "class census: {} unported class-levels ({} placed, {} created instances), {} class-port units, {} shared functions; {} ported units, {} mode roots -> {}",
                s.classes, s.placed, s.created, s.units, s.shared, s.ported_units, s.roots, s.out.display()
            );
            Ok(0)
        }
        "info" => {
            let p = ee::resolve_state(&a.opt("--state")?.context("--state required")?)?;
            a.done()?;
            let s = Savestate::open(&p)?;
            println!("{}", p.display());
            if let Ok(v) = s.version() { println!("  save version {:#010x} (major {:#06x}), PCSX2 {}", v.save_version, v.save_version >> 16, v.build); }
            for e in &s.archive.entries {
                println!("  {:<34} method {:>2}  {:>10} -> {:>10} bytes", e.name, e.method, e.compressed_size, e.size);
            }
            Ok(0)
        }
        "dump-ee" => {
            let out = a.opt("--out")?.map(PathBuf::from);
            let entry = a.opt("--entry")?;
            let src = a.source()?;
            a.done()?;
            let out = out.unwrap_or_else(|| {
                let stem = match &src { EeSource::State(p) | EeSource::Raw(p) => p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), EeSource::Pine(_) => "pine".into() };
                trace_out_dir().join(match &entry { Some(e) => format!("{stem}_{e}"), None => format!("{stem}_ee.bin") })
            });
            let bytes = match (entry, &src) {
                (Some(e), EeSource::State(p)) => Savestate::open(p)?.archive.read(&e)?,
                (Some(_), _) => bail!("--entry needs --state"),
                (None, s) => s.load()?.ram,
            };
            write_output(&out, &bytes)?;
            println!("wrote {} ({} bytes)", out.display(), bytes.len());
            Ok(0)
        }
        "find" => {
            let hex = a.opt("--bytes")?.context("--bytes required")?;
            let align: usize = a.opt("--align")?.map(|s| s.parse()).transpose()?.unwrap_or(2);
            let max: usize = a.opt("--max")?.map(|s| s.parse()).transpose()?.unwrap_or(64);
            let src = a.source()?;
            a.done()?;
            let clean: String = hex.chars().filter(|c| c.is_ascii_hexdigit()).collect();
            if !clean.len().is_multiple_of(2) || clean.is_empty() { bail!("--bytes needs an even number of hex digits"); }
            let pat: Vec<u8> = (0..clean.len()).step_by(2).map(|i| u8::from_str_radix(&clean[i..i + 2], 16).unwrap()).collect();
            let img = src.load()?;
            let hits = img.find(&pat, align);
            println!("{} hit(s) for {} bytes (align {align})", hits.len(), pat.len());
            for h in hits.iter().take(max) { println!("  {h:#010x}"); }
            Ok(0)
        }
        "read" => {
            let addr = hex_u32(&a.opt("--addr")?.context("--addr required")?)?;
            let len: usize = a.opt("--len")?.map(|s| s.parse()).transpose()?.unwrap_or(0x40);
            let src = a.source()?;
            a.done()?;
            let img = src.load()?;
            for (k, row) in img.bytes(addr, len)?.chunks(16).enumerate() {
                let hex: Vec<String> = row.iter().map(|b| format!("{b:02x}")).collect();
                println!("{:#010x}: {}", addr as usize + 16 * k, hex.join(" "));
            }
            Ok(0)
        }
        "pine-info" => {
            let slot = a.0.first().and_then(|s| s.parse().ok()).unwrap_or(pine::DEFAULT_SLOT);
            let mut c = pine::Pine::connect(slot)?;
            println!("socket  {}", pine::endpoint(slot));
            println!("version {}", c.version()?);
            println!("status  {}", c.status_name());
            println!("game    {} | {}", c.game_id().unwrap_or_default(), c.title().unwrap_or_default());
            Ok(0)
        }
        "pine-write" => {
            let slot: u16 = a.opt("--slot")?.map(|s| s.parse()).transpose()?.unwrap_or(pine::DEFAULT_SLOT);
            let addr = hex_u32(&a.opt("--addr")?.context("--addr required")?)?;
            let hex = a.opt("--bytes")?.context("--bytes required")?;
            a.done()?;
            if !hex.len().is_multiple_of(2) { bail!("--bytes: an even number of hex digits"); }
            let bytes = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).with_context(|| format!("bad hex {hex:?}"))).collect::<Result<Vec<_>>>()?;
            pine::Pine::connect(slot)?.write(addr, &bytes)?;
            println!("wrote {} bytes at {addr:#x}", bytes.len());
            Ok(0)
        }
        "pine-savestate" => {
            let slot: u16 = a.opt("--slot")?.map(|s| s.parse()).transpose()?.unwrap_or(pine::DEFAULT_SLOT);
            let state: u8 = a.0.first().context("state slot number required")?.parse()?;
            pine::Pine::connect(slot)?.save_state(state)?;
            println!("requested save to state slot {state}; the file appears in {}", ee::pcsx2_sstates_dir().unwrap_or_default().display());
            Ok(0)
        }
        _ => {
            eprintln!("unknown command {cmd:?}\n\n{USAGE}");
            Ok(2)
        }
    }
}
