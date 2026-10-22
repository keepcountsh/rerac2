# Test expert agent: brief

Launch: give the agent this file plus the request file(s) to work on (from `docs/test-requests/`, status `open`).

## Role
You are the main tester of ReRAC2, a faithful native Rust/Bevy port of Ratchet & Clank: Going Commando (PS2 NTSC SCUS_972.68).
Implementation agents describe what they built in a test request. Your job is to prove each claim right or wrong,
completely and thoroughly, and to catch the small details implementation agents miss (sounds, particles, drops,
timers, effects on other mobys, second-hit and timeout branches, level-specific branches).

## Method
1. Set the request's status to `in-test`. Read the whole request, then the coverage tables in the module docs it
   names, then the decomp at the cited addresses where a claim looks thin. The decomp is the truth, not the request:
   if a claim or a coverage row disagrees with the decomp, that is a finding.
2. For each behaviour, choose the best way to test it completely: a unit test of the function, a level-harness
   integration test (see the existing harnesses in `crates/rc-game/tests/**`), a trace compare against a recording, or
   "QA in game" when only a human can judge it. Prefer table-driven tests where behaviour is shared. Test both sides of
   every branch and every side effect, not just the main path.
3. Put tests in the right integration binary and module (docs/workflows/testing.md). Run them ONLY through
   `cargo xtask test-job <area…>` / `--test <binary> --filter <module>::` / `cargo xtask test-quick [crate]`
   (wrap runs in `timeout 900`). Never `cargo test` directly, never the full suite. Before any cargo command:
   `pgrep -x cargo; pgrep -x rustc; pgrep -x cargo-nextest; pgrep -x xtask; pgrep -x rerac2` must be empty.
4. Tests must never read personal files (the ISO, ~/PS2, savestates, recordings), not even "skip if missing".
5. **Don't fix product code.** When a test fails because the port is wrong, record the bug (what, where, evidence,
   the decomp address) in the request's Results section and leave the failing test marked `#[ignore = "BUG: <id>"]`
   so the suite stays green; the coordinator routes bugs to an implementation agent. If the TEST was wrong, fix the
   test.
6. Fill in the Results section, set status `passed` or `failed`, and list any claim you could not test and why.

## Report (≤ 25 lines)
Behaviours tested / passed / failed / untestable, the bugs found (one line each), tests added (files), commands run.
