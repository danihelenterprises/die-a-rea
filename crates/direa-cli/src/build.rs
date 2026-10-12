//! Built-in hardening steps: artifact-level scrubbing plus the randomized
//! decoy and tarpit layer.
//!
//! Two ideas drive this module:
//!
//! * **Randomization** — inject seeded, reproducible decoys (extra "jigsaw
//!   pieces") so automated analysis recovers far more than it can trust.
//! * **Tarpits** — inflate the cost of automated / LLM analysis with padding,
//!   depth bombs, honeypots and an authorization gate.
//!
//! Everything is driven by a seeded [`Rng`](direa_core::Rng), so a given seed
//! reproduces a given layout exactly.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use direa_core::native_post::{strip_symbols, StripReport};
use direa_core::planner::detect_kind;
use direa_core::random::derive_seed;
use direa_core::{
    BuildMetadata, InjectedStats, Manifest, Randomization, ResolvedPolicy, Rng, TargetKind,
    TargetMetadata, Tarpit,
};
use serde::Serialize;

pub const DEFAULT_NOTICE: &str =
    "This artifact is protected by die-a-rea. Automated reverse engineering requires written authorization.";

const MAX_DECOY_FUNCTIONS: u32 = 20_000;
const MAX_DECOY_STRINGS: u32 = 50_000;
const MAX_DECOY_FILES: u32 = 2_000;
const MAX_DEPTH: u32 = 40;

#[derive(Debug, Serialize)]
pub struct BuildResult {
    pub output_dir: PathBuf,
    pub seed: u64,
    pub targets: Vec<TargetResult>,
    pub notes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct TargetResult {
    pub target: String,
    pub kind: String,
    pub output: String,
    pub canaries: Vec<String>,
    pub actions: Vec<String>,
    pub injected: InjectedStats,
    pub noise_fraction: f64,
}

/// Resolve the canary tokens to embed for this build.
pub fn resolve_canaries(manifest: &Manifest, policy: &ResolvedPolicy) -> Vec<String> {
    if !manifest.agent_defense.canaries.is_empty() {
        return manifest
            .agent_defense
            .canaries
            .iter()
            .map(|c| {
                if c.starts_with("DIREA-CANARY-") {
                    c.clone()
                } else {
                    format!("DIREA-CANARY-{c}")
                }
            })
            .collect();
    }
    if policy.agent_canaries {
        let mut h = DefaultHasher::new();
        manifest.project.name.hash(&mut h);
        manifest.project.version.hash(&mut h);
        return vec![format!("DIREA-CANARY-{:016x}", h.finish())];
    }
    Vec::new()
}

pub fn build(
    manifest: &Manifest,
    out: &Path,
    dry_run: bool,
    seed_override: Option<u64>,
) -> Result<BuildResult> {
    let policy = manifest.resolved_policy();
    let canaries = resolve_canaries(manifest, &policy);
    let notice = manifest
        .agent_defense
        .notice_text
        .clone()
        .unwrap_or_else(|| DEFAULT_NOTICE.to_string());

    let base_seed = seed_override.unwrap_or(manifest.randomization.seed);
    let rz = manifest.randomization.clone();
    let tp = manifest.tarpit.clone();

    let mut result = BuildResult {
        output_dir: out.to_path_buf(),
        seed: base_seed,
        targets: Vec::new(),
        notes: Vec::new(),
    };
    let mut meta = BuildMetadata {
        seed: base_seed,
        targets: Vec::new(),
    };

    if manifest.targets.is_empty() {
        result
            .notes
            .push("no targets configured; nothing to do".into());
        return Ok(result);
    }

    for spec in &manifest.targets {
        let src = &spec.path;
        if !src.exists() {
            result
                .notes
                .push(format!("skipping missing target: {}", src.display()));
            continue;
        }

        let bytes = std::fs::read(src)
            .ok()
            .and_then(|b| b.get(..b.len().min(4096)).map(|s| s.to_vec()))
            .unwrap_or_default();
        let kind = spec.kind.unwrap_or_else(|| detect_kind(src, &bytes));

        let seed = if base_seed != 0 {
            base_seed
        } else {
            derive_seed(&[
                &manifest.project.name,
                &manifest.project.version,
                &src.display().to_string(),
            ])
        };
        let mut rng = Rng::new(seed);

        let rel = relative_dest(src);
        let dest = out.join(&rel);
        let mut actions = Vec::new();
        let mut injected = InjectedStats::default();

        match kind {
            TargetKind::JavaScript | TargetKind::ElectronAsar if src.is_file() => {
                actions.push("strip source maps".into());
                actions.push("strip full-line comments".into());
                if policy.agent_stop_notice {
                    actions.push("inject authorization notice".into());
                }
                if !canaries.is_empty() {
                    actions.push(format!("embed {} canary token(s)", canaries.len()));
                }
                injected = plan_decoys(&policy, &rz, &tp, &mut actions, true);
                if !dry_run {
                    ensure_parent(&dest)?;
                    injected = harden_text_file(
                        src, &dest, &policy, &notice, &canaries, &rz, &tp, &mut rng,
                    )?;
                }
            }
            TargetKind::JavaScript | TargetKind::ElectronAsar => {
                actions.push("copy tree; strip *.map and inline source maps".into());
                if policy.agent_stop_notice {
                    actions.push("inject authorization notice".into());
                }
                injected = plan_decoys(&policy, &rz, &tp, &mut actions, false);
                if !dry_run {
                    std::fs::create_dir_all(&dest)
                        .with_context(|| format!("creating {}", dest.display()))?;
                    harden_tree(src, &dest, &policy, &notice, &canaries)?;
                    injected = write_decoy_tree(&dest, &policy, &rz, &tp, &notice, &mut rng)?;
                }
            }
            TargetKind::NativeElf
            | TargetKind::NativeMachO
            | TargetKind::NativePe
            | TargetKind::Go => {
                actions.push("native-post: strip symbols/debug/build metadata".into());
                if policy.tarpit {
                    actions.push("append tarpit blob (re-sign required)".into());
                }
                result.notes.push(
                    "note: deeper native transforms (control-flow) require the `llvm` adapter"
                        .into(),
                );
                if !dry_run {
                    ensure_parent(&dest)?;
                    let rep =
                        harden_native(src, &dest, &policy, &tp, &notice, &mut rng, &mut injected)?;
                    result.notes.push(format!(
                        "native-post[{}]: removed {} structure(s), zeroed {} bytes",
                        rep.format,
                        rep.removed.len(),
                        rep.bytes_zeroed
                    ));
                } else {
                    injected.padding_bytes = if policy.tarpit {
                        tp.padding_kb.min(tp.max_total_kb) as u64 * 1024
                    } else {
                        0
                    };
                    injected.injected_bytes = injected.padding_bytes;
                }
            }
            _ => {
                // Unknown containers still get a best-effort native strip and an
                // optional tarpit append.
                actions.push("copy artifact; best-effort native-post".into());
                if policy.tarpit {
                    actions.push("append tarpit blob (re-sign required)".into());
                }
                if !dry_run {
                    ensure_parent(&dest)?;
                    let rep =
                        harden_native(src, &dest, &policy, &tp, &notice, &mut rng, &mut injected)?;
                    if rep.changed {
                        result.notes.push(format!(
                            "native-post[{}]: removed {} structure(s)",
                            rep.format,
                            rep.removed.len()
                        ));
                    }
                } else if policy.tarpit {
                    injected.padding_bytes = tp.padding_kb.min(tp.max_total_kb) as u64 * 1024;
                    injected.injected_bytes = injected.padding_bytes;
                }
            }
        }

        let total_bytes = if dry_run {
            file_or_dir_size(src).unwrap_or(0) + injected.injected_bytes
        } else {
            file_or_dir_size(&dest).unwrap_or(0)
        };
        let record = TargetMetadata {
            target: src.display().to_string(),
            output: dest.display().to_string(),
            total_bytes,
            injected: injected.clone(),
        };
        let noise_fraction = record.noise_fraction();
        meta.targets.push(record);

        result.targets.push(TargetResult {
            target: src.display().to_string(),
            kind: kind.as_str().to_string(),
            output: dest.display().to_string(),
            canaries: canaries.clone(),
            actions,
            injected,
            noise_fraction,
        });
    }

    if !dry_run {
        std::fs::create_dir_all(out).ok();
        let path = out.join(direa_core::buildmeta::BUILD_METADATA_FILE);
        meta.save(&path)
            .with_context(|| format!("writing {}", path.display()))?;
        result
            .notes
            .push(format!("wrote build metadata to {}", path.display()));
    }

    Ok(result)
}

fn plan_decoys(
    policy: &ResolvedPolicy,
    rz: &Randomization,
    tp: &Tarpit,
    actions: &mut Vec<String>,
    inline: bool,
) -> InjectedStats {
    let mut stats = InjectedStats::default();
    if policy.randomize {
        let fns = rz.decoy_functions.min(MAX_DECOY_FUNCTIONS);
        let strings = rz.decoy_strings.min(MAX_DECOY_STRINGS);
        actions.push(format!(
            "inject {fns} decoy function(s) and {strings} decoy string(s)"
        ));
        stats.decoy_functions = fns as u64;
        stats.decoy_strings = strings as u64;
        if rz.depth_bomb > 0 {
            stats.depth_bytes = (rz.depth_bomb.min(MAX_DEPTH) * 3) as u64 * 64;
        }
        if !inline {
            stats.decoy_files = rz.decoy_files.min(MAX_DECOY_FILES) as u64;
        }
    }
    if policy.tarpit {
        actions.push(format!(
            "add tarpit padding (~{} KiB, cap {} KiB) and {} honeypot(s)",
            tp.padding_kb, tp.max_total_kb, tp.honeypots
        ));
        stats.padding_bytes = tp.padding_kb.min(tp.max_total_kb) as u64 * 1024;
    }
    stats.injected_bytes = stats.padding_bytes + stats.depth_bytes;
    stats
}

fn merge(into: &mut InjectedStats, other: &InjectedStats) {
    into.decoy_functions += other.decoy_functions;
    into.decoy_strings += other.decoy_strings;
    into.decoy_files += other.decoy_files;
    into.injected_string_runs += other.injected_string_runs;
    into.depth_bytes += other.depth_bytes;
    into.padding_bytes += other.padding_bytes;
    into.injected_bytes += other.injected_bytes;
}

/// Count printable sequences (>= `min` chars), matching the verify `strings`
/// probe so injected content can be discounted exactly.
fn printable_runs(bytes: &[u8], min: usize) -> u64 {
    let mut runs = 0u64;
    let mut len = 0usize;
    for &b in bytes {
        if (0x20..=0x7e).contains(&b) {
            len += 1;
        } else {
            if len >= min {
                runs += 1;
            }
            len = 0;
        }
    }
    if len >= min {
        runs += 1;
    }
    runs
}

fn ensure_parent(dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
    }
    Ok(())
}

fn relative_dest(src: &Path) -> PathBuf {
    if src.is_absolute() {
        src.file_name()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("artifact"))
    } else {
        src.to_path_buf()
    }
}

#[allow(clippy::too_many_arguments)]
fn harden_text_file(
    src: &Path,
    dest: &Path,
    policy: &ResolvedPolicy,
    notice: &str,
    canaries: &[String],
    rz: &Randomization,
    tp: &Tarpit,
    rng: &mut Rng,
) -> Result<InjectedStats> {
    let text = std::fs::read_to_string(src)
        .with_context(|| format!("reading {} as UTF-8", src.display()))?;
    let (body, _comments, _maps) = transform_js(&text, policy);

    let mut out = String::new();
    if policy.agent_stop_notice {
        out.push_str("/* ");
        out.push_str(&sanitize_comment(notice));
        out.push_str(" */\n");
    }
    for (i, canary) in canaries.iter().enumerate() {
        out.push_str(&format!("/* direa-canary-{i}: {canary} */\n"));
    }
    out.push_str(&body);

    let mut stats = InjectedStats::default();
    if policy.randomize || policy.tarpit {
        let (decoy, s) = generate_js_bundle(rng, policy, rz, tp, notice);
        out.push('\n');
        out.push_str(&decoy);
        stats = s;
    }

    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).ok();
        }
    }
    std::fs::write(dest, out).with_context(|| format!("writing {}", dest.display()))?;
    Ok(stats)
}

/// Returns (transformed body, comments stripped, source maps stripped).
fn transform_js(text: &str, policy: &ResolvedPolicy) -> (String, usize, usize) {
    let mut comments = 0;
    let mut maps = 0;
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//# sourceMappingURL=")
            || trimmed.starts_with("//@ sourceMappingURL=")
            || trimmed.contains("sourceMappingURL=")
        {
            maps += 1;
            continue;
        }
        if policy.strip_build_metadata && trimmed.starts_with("//") && !line.contains('`') {
            comments += 1;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    (out, comments, maps)
}

fn harden_tree(
    src: &Path,
    dest: &Path,
    policy: &ResolvedPolicy,
    notice: &str,
    canaries: &[String],
) -> Result<()> {
    let entries = std::fs::read_dir(src).with_context(|| format!("reading {}", src.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let target = dest.join(&name);
        if path.is_dir() {
            std::fs::create_dir_all(&target).ok();
            harden_tree(&path, &target, policy, notice, canaries)?;
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        if ext == "map" {
            continue;
        }
        if matches!(ext.as_str(), "js" | "mjs" | "cjs") {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let (body, _c, _m) = transform_js(&text, policy);
            let mut out = String::new();
            if policy.agent_stop_notice {
                out.push_str(&format!("/* {} */\n", sanitize_comment(notice)));
            }
            for (i, canary) in canaries.iter().enumerate() {
                out.push_str(&format!("/* direa-canary-{i}: {canary} */\n"));
            }
            out.push_str(&body);
            std::fs::write(&target, out)
                .with_context(|| format!("writing {}", target.display()))?;
        } else {
            std::fs::copy(&path, &target).with_context(|| format!("copying {}", path.display()))?;
        }
    }
    if policy.agent_stop_notice {
        let notice_path = dest.join("DIREA-NOTICE.txt");
        let _ = std::fs::write(notice_path, format!("{notice}\n"));
    }
    Ok(())
}

/// Write decoy files and tarpit bundles into a hardened tree.
fn write_decoy_tree(
    dest: &Path,
    policy: &ResolvedPolicy,
    rz: &Randomization,
    tp: &Tarpit,
    notice: &str,
    rng: &mut Rng,
) -> Result<InjectedStats> {
    let mut stats = InjectedStats::default();
    if !(policy.randomize || policy.tarpit) {
        return Ok(stats);
    }
    let dir = dest.join(format!("__{}", rng.ident(8)));
    std::fs::create_dir_all(&dir).ok();

    if policy.randomize {
        let files = rz.decoy_files.clamp(1, MAX_DECOY_FILES);
        let small = Randomization {
            decoy_functions: (rz.decoy_functions / files).max(1),
            decoy_strings: (rz.decoy_strings / files).max(1),
            depth_bomb: rz.depth_bomb,
            ..rz.clone()
        };
        let no_tarpit = Tarpit {
            enabled: false,
            ..tp.clone()
        };
        for i in 0..files {
            let (bundle, st) = generate_js_bundle(rng, policy, &small, &no_tarpit, notice);
            merge(&mut stats, &st);
            let name = format!("{}_{}.js", rng.word_ident(), i);
            std::fs::write(dir.join(&name), bundle).ok();
        }
        stats.decoy_files = files as u64;
    }

    if policy.tarpit {
        let (padding, bytes) = generate_padding(rng, tp.padding_kb, tp.max_total_kb, notice);
        stats.injected_string_runs += printable_runs(padding.as_bytes(), 6);
        std::fs::write(dir.join("data.js"), &padding).ok();
        stats.padding_bytes += bytes;
        stats.injected_bytes += bytes;
    }
    Ok(stats)
}

fn harden_native(
    src: &Path,
    dest: &Path,
    policy: &ResolvedPolicy,
    tp: &Tarpit,
    notice: &str,
    rng: &mut Rng,
    stats: &mut InjectedStats,
) -> Result<StripReport> {
    let mut data = std::fs::read(src).with_context(|| format!("reading {}", src.display()))?;
    let report = if policy.strip_symbols {
        strip_symbols(&mut data)
    } else {
        StripReport::default()
    };
    if policy.tarpit {
        let (blob, bytes) = generate_padding(rng, tp.padding_kb, tp.max_total_kb, notice);
        data.extend_from_slice(b"\n/* ");
        data.extend_from_slice(notice.as_bytes());
        data.extend_from_slice(b" */\n");
        data.extend_from_slice(blob.as_bytes());
        stats.padding_bytes = bytes + notice.len() as u64 + 8;
        stats.injected_bytes += stats.padding_bytes;
        stats.injected_string_runs =
            printable_runs(&data[data.len() - stats.padding_bytes as usize..], 6);
    }
    std::fs::write(dest, data).with_context(|| format!("writing {}", dest.display()))?;
    Ok(report)
}

/// Generate an appended JavaScript decoy + tarpit bundle.
fn generate_js_bundle(
    rng: &mut Rng,
    policy: &ResolvedPolicy,
    rz: &Randomization,
    tp: &Tarpit,
    notice: &str,
) -> (String, InjectedStats) {
    let mut out = String::new();
    let mut stats = InjectedStats::default();
    let before = |s: &str| s.len() as u64;

    if policy.agent_stop_notice && tp.prompt_gate && tp.enabled {
        let id = rng.ident(10);
        out.push_str(&format!("var {id} = \"{}\";\n", sanitize_string(notice)));
    }

    if policy.randomize {
        let fns = rz.decoy_functions.min(MAX_DECOY_FUNCTIONS);
        for _ in 0..fns {
            out.push_str(&gen_function(rng));
        }
        stats.decoy_functions += fns as u64;

        let strings = rz.decoy_strings.min(MAX_DECOY_STRINGS);
        for _ in 0..strings {
            let id = rng.ident(9);
            out.push_str(&format!("var {id} = \"{}\";\n", rng.decoy_string()));
        }
        stats.decoy_strings += strings as u64;

        let depth = rz.depth_bomb.min(MAX_DEPTH);
        if depth > 0 {
            for _ in 0..3 {
                let bomb = gen_depth_bomb(rng, depth);
                out.push_str(&bomb);
            }
            stats.depth_bytes += out.len() as u64 / 8;
        }
    }

    if policy.tarpit && tp.enabled {
        for _ in 0..tp.honeypots {
            out.push_str(&gen_honeypot(rng));
        }
        for payload in &tp.custom_payloads {
            out.push_str(&format!("/* {} */\n", sanitize_comment(payload)));
        }
        let (padding, bytes) = generate_padding(rng, tp.padding_kb, tp.max_total_kb, notice);
        out.push_str(&padding);
        stats.padding_bytes += bytes;
    }

    stats.injected_bytes = before(&out);
    stats.injected_string_runs = printable_runs(out.as_bytes(), 6);
    (out, stats)
}

fn gen_function(rng: &mut Rng) -> String {
    let name = rng.word_ident();
    let p1 = rng.ident(4);
    let p2 = rng.ident(5);
    let v = rng.ident(3);
    let k = rng.range(2, 9999);
    let m = rng.range(3, 251);
    let s = rng.decoy_string();
    format!(
        "function {name}({p1}, {p2}) {{ var {v} = (({p1} * {k}) + {p2}) & {m}; if (((({v} ^ {k}) & 1) === -1)) {{ return \"{s}\"; }} return {v}; }}\n"
    )
}

fn gen_honeypot(rng: &mut Rng) -> String {
    let verb = rng.pick(direa_core::random::WORDLIST);
    let noun = rng.pick(direa_core::random::WORDLIST);
    let suffix = rng.hex(4);
    let s = rng.decoy_string();
    let arg = rng.ident(5);
    format!("function {verb}_{noun}_{suffix}({arg}) {{ var k = \"{s}\"; return {arg} === k; }}\n")
}

fn gen_depth_bomb(rng: &mut Rng, depth: u32) -> String {
    let names: Vec<String> = (0..=depth).map(|_| rng.ident(6)).collect();
    let mut body = format!("return {} + 1;", rng.range(0, 9));
    for i in (0..depth as usize).rev() {
        let arg = rng.range(0, 9);
        body = format!(
            "return (function {}(x) {{ {} }})({});",
            names[i + 1],
            body,
            arg
        );
    }
    format!("function {}(x) {{ {} }}\n", names[0], body)
}

fn generate_padding(rng: &mut Rng, kb: u32, cap_kb: u32, notice: &str) -> (String, u64) {
    let kb = kb.min(cap_kb) as usize;
    let target = kb * 1024;
    let mut s = String::with_capacity(target + 64);
    s.push_str("/* ANALYST NOTICE: ");
    s.push_str(&sanitize_comment(notice));
    s.push('\n');
    while s.len() < target {
        s.push_str(&rng.phrase(12));
        s.push('\n');
    }
    s.push_str("*/\n");
    let len = s.len() as u64;
    (s, len)
}

fn sanitize_comment(text: &str) -> String {
    text.replace("*/", "* /")
}

fn sanitize_string(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn file_or_dir_size(path: &Path) -> Option<u64> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.is_file() {
        return Some(meta.len());
    }
    let mut total = 0;
    let entries = std::fs::read_dir(path).ok()?;
    for entry in entries.flatten() {
        total += file_or_dir_size(&entry.path()).unwrap_or(0);
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_source_map_lines() {
        let text = "var x=1;\n//# sourceMappingURL=app.js.map\nvar y=2;\n";
        let (out, _comments, maps) = transform_js(text, &ResolvedPolicy::default());
        assert!(!out.contains("sourceMappingURL"));
        assert_eq!(maps, 1);
        assert!(out.contains("var y=2;"));
    }

    #[test]
    fn canaries_are_namespaced() {
        let mut m = Manifest::default();
        m.agent_defense.canaries = vec!["lic-123".into()];
        let p = m.resolved_policy();
        let c = resolve_canaries(&m, &p);
        assert_eq!(c, vec!["DIREA-CANARY-lic-123".to_string()]);
    }

    #[test]
    fn decoy_bundle_is_reproducible() {
        let policy = direa_core::Profile::High.resolve();
        let rz = Randomization {
            decoy_functions: 10,
            decoy_strings: 10,
            depth_bomb: 2,
            ..Default::default()
        };
        let tp = Tarpit {
            padding_kb: 1,
            honeypots: 3,
            ..Default::default()
        };
        let (a, _) = generate_js_bundle(&mut Rng::new(99), &policy, &rz, &tp, "notice");
        let (b, _) = generate_js_bundle(&mut Rng::new(99), &policy, &rz, &tp, "notice");
        assert_eq!(a, b);
        assert!(a.contains("function "));
    }

    #[test]
    fn bundle_reports_injected_runs() {
        let policy = direa_core::Profile::High.resolve();
        let rz = Randomization {
            decoy_functions: 5,
            decoy_strings: 5,
            depth_bomb: 0,
            ..Default::default()
        };
        let tp = Tarpit {
            padding_kb: 1,
            honeypots: 0,
            ..Default::default()
        };
        let (_, st) = generate_js_bundle(&mut Rng::new(1), &policy, &rz, &tp, "n");
        assert!(st.injected_string_runs > 0);
        assert!(st.injected_bytes > 0);
        assert!(st.padding_bytes > 0);
    }

    #[test]
    fn padding_is_bounded() {
        let (_s, bytes) = generate_padding(&mut Rng::new(1), 999_999, 16, "n");
        assert!(bytes <= 16 * 1024 + 128);
    }

    #[test]
    fn comment_sanitized() {
        assert_eq!(sanitize_comment("a */ b"), "a * / b");
    }
}
