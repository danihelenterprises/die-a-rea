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

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use direa_core::adapter::{AdapterOp, AdapterRequest, SubprocessAdapter};
use direa_core::native_post::{strip_symbols, StripReport};
use direa_core::planner::detect_kind;
use direa_core::random::derive_seed;
use direa_core::AdapterEntry;
use direa_core::{
    BuildMetadata, InjectedStats, Manifest, Randomization, ResolvedPolicy, Rng, TargetKind,
    TargetMetadata, Tarpit,
};
use serde::Serialize;

/// Build a coded refusal so the CLI can emit a stable error code (B-029).
fn refused(code: &'static str, message: String) -> anyhow::Error {
    direa_core::DireaError::Refused { code, message }.into()
}

const MAX_DECOY_FUNCTIONS: u32 = 20_000;
const MAX_DECOY_STRINGS: u32 = 50_000;
const MAX_DECOY_FILES: u32 = 2_000;
const MAX_DEPTH: u32 = 40;
/// Operator-supplied deceptive payloads are opt-in and hard-bounded (ADR-0016).
const MAX_CUSTOM_PAYLOADS: usize = 32;
const MAX_CUSTOM_PAYLOAD_LEN: usize = 4096;

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
        return vec![direa_core::canary::canary_token(
            &manifest.project.name,
            &manifest.project.version,
        )];
    }
    Vec::new()
}

pub fn build(
    manifest: &Manifest,
    out: &Path,
    dry_run: bool,
    seed_override: Option<u64>,
    force: bool,
) -> Result<BuildResult> {
    let policy = manifest.resolved_policy();
    let canaries = resolve_canaries(manifest, &policy);
    let base_seed = seed_override.unwrap_or(manifest.randomization.seed);
    let notice = manifest
        .agent_defense
        .notice_text
        .clone()
        .unwrap_or_else(|| direa_core::canary::gate_text(base_seed));
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

    // Safety: never write into a directory die-a-rea did not create (unless
    // explicitly forced), so an existing tree is not clobbered.
    if !dry_run && !force && out.exists() {
        let has_sidecar = out
            .join(direa_core::buildmeta::BUILD_METADATA_FILE)
            .exists();
        let non_empty = std::fs::read_dir(out)
            .map(|mut d| d.next().is_some())
            .unwrap_or(false);
        if non_empty && !has_sidecar {
            return Err(refused(
                "output_dir_foreign",
                format!(
                    "output directory {} already exists and was not created by die-a-rea; pass --force to write into it (or choose a fresh --out)",
                    out.display()
                ),
            ));
        }
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
        guard_source_and_output(src, &dest)?;
        let mut actions = Vec::new();
        let mut injected = InjectedStats::default();

        match kind {
            TargetKind::JavaScript if src.is_file() => {
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
                    apply_external_adapters(&dest, &policy, kind, &mut actions, &mut result.notes);
                }
            }
            TargetKind::ElectronAsar if src.is_file() => {
                // A single-file `.asar` is a binary archive: rewrite the embedded
                // JS in place (source-map/comment strip, notice, canaries, guard)
                // and rebuild the archive; fall back to a verbatim copy when the
                // archive is not parseable (B-055).
                if dry_run {
                    actions.push("asar: rewrite embedded JS, drop per-file integrity".into());
                } else {
                    ensure_parent(&dest)?;
                    let data =
                        std::fs::read(src).with_context(|| format!("reading {}", src.display()))?;
                    let transform = |text: &str| harden_js_text(text, &policy, &notice, &canaries);
                    let (new_data, rep) = direa_core::asar::harden_archive(&data, &transform);
                    std::fs::write(&dest, &new_data)
                        .with_context(|| format!("writing {}", dest.display()))?;
                    if rep.changed {
                        actions.push(format!(
                            "asar: rewrote {} of {} embedded file(s)",
                            rep.files_hardened, rep.files_total
                        ));
                        result.notes.push(format!(
                            "asar: hardened {} embedded JS file(s), {} file(s) total",
                            rep.files_hardened, rep.files_total
                        ));
                    } else {
                        actions.push("copy .asar archive verbatim (not a parseable asar)".into());
                        apply_external_adapters(
                            &dest,
                            &policy,
                            kind,
                            &mut actions,
                            &mut result.notes,
                        );
                    }
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
                    apply_external_adapters(&dest, &policy, kind, &mut actions, &mut result.notes);
                }
            }
            TargetKind::Wasm => {
                actions.push("wasm-post: strip custom sections".into());
                if !dry_run {
                    ensure_parent(&dest)?;
                    let mut data =
                        std::fs::read(src).with_context(|| format!("reading {}", src.display()))?;
                    let rep = direa_core::wasm::strip_custom_sections(&mut data);
                    std::fs::write(&dest, data)
                        .with_context(|| format!("writing {}", dest.display()))?;
                    result.notes.push(format!(
                        "wasm-post: cleared {} custom section(s)",
                        rep.removed.len()
                    ));
                    apply_external_adapters(&dest, &policy, kind, &mut actions, &mut result.notes);
                }
            }
            TargetKind::JavaClass => {
                actions.push("java-post: strip debug attributes".into());
                if !dry_run {
                    ensure_parent(&dest)?;
                    let mut data =
                        std::fs::read(src).with_context(|| format!("reading {}", src.display()))?;
                    let rep = direa_core::java::strip_debug_attributes(&mut data);
                    std::fs::write(&dest, data)
                        .with_context(|| format!("writing {}", dest.display()))?;
                    result.notes.push(format!(
                        "java-post: cleared {} debug attribute(s)",
                        rep.removed.len()
                    ));
                    apply_external_adapters(&dest, &policy, kind, &mut actions, &mut result.notes);
                }
            }
            TargetKind::JavaJar => {
                // A `.jar` is a zip container; the built-in `java-post` works on
                // `.class` files, not archives. Copy it verbatim and defer to an
                // external shrinker (the `jvm` adapter: R8/ProGuard). Built-in
                // jar unpack/harden/repack is not implemented.
                actions.push(
                    "copy .jar archive verbatim (use the jvm adapter for R8/ProGuard)".into(),
                );
                if !dry_run {
                    ensure_parent(&dest)?;
                    std::fs::copy(src, &dest).with_context(|| {
                        format!("copying {} to {}", src.display(), dest.display())
                    })?;
                    apply_external_adapters(&dest, &policy, kind, &mut actions, &mut result.notes);
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
                    apply_external_adapters(&dest, &policy, kind, &mut actions, &mut result.notes);
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
                    apply_external_adapters(&dest, &policy, kind, &mut actions, &mut result.notes);
                } else if policy.tarpit {
                    injected.padding_bytes = tp.padding_kb.min(tp.max_total_kb) as u64 * 1024;
                    injected.injected_bytes = injected.padding_bytes;
                }
            }
        }

        if !dry_run {
            post_build_check(src, &dest, kind, out, &manifest.verify)?;
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

/// Count tokens in injected content. Informational: the `strings` axis now
/// measures verbatim survival of the baseline's tokens (ADR-0011), so this no
/// longer discounts anything, but it is retained in the build metadata.
fn token_runs(bytes: &[u8]) -> u64 {
    direa_core::scan::count_tokens(bytes, direa_core::scan::DEFAULT_MIN_TOKEN)
}

use direa_core::adapter::env_key as adapter_env_key;

/// Resolve an external adapter: env override first, then its command on `PATH`.
fn resolve_adapter(id: &str, command: &str) -> Option<AdapterEntry> {
    if let Ok(path) = std::env::var(adapter_env_key(id)) {
        if !path.trim().is_empty() {
            return Some(AdapterEntry {
                id: id.into(),
                command: path,
                args: vec![],
                kinds: vec![],
                builtin: false,
            });
        }
    }
    direa_core::which(command).map(|p| AdapterEntry {
        id: id.into(),
        command: p.to_string_lossy().into_owned(),
        args: vec![],
        kinds: vec![],
        builtin: false,
    })
}

/// Run every external adapter registered for this target kind, if installed.
/// Absence or failure is a note, never an error (ADR-0009).
fn apply_external_adapters(
    dest: &Path,
    policy: &ResolvedPolicy,
    kind: TargetKind,
    actions: &mut Vec<String>,
    notes: &mut Vec<String>,
) {
    for entry in direa_core::planner::default_registry()
        .iter()
        .filter(|e| !e.builtin && e.kinds.contains(&kind))
    {
        let Some(resolved) = resolve_adapter(&entry.id, &entry.command) else {
            notes.push(format!(
                "{} adapter not installed (set {} or add `{}` to PATH)",
                entry.id,
                adapter_env_key(&entry.id),
                entry.command
            ));
            continue;
        };
        actions.push(format!("{} adapter: {}", entry.id, resolved.command));
        let request = AdapterRequest {
            protocol: direa_core::adapter::proto_version(),
            op: AdapterOp::Apply,
            target: dest.to_path_buf(),
            kind,
            policy: policy.clone(),
            // `{artifact}` = the target (built artifact); `{out}` = where a
            // post-processor should write. Both resolve to the hardened artifact
            // so configured commands work in place (ADR-0009).
            options: serde_json::json!({ "out": dest }),
        };
        match SubprocessAdapter::new(resolved).run(&request) {
            Ok(resp) => {
                for step in &resp.steps {
                    actions.push(format!("{}: {}", entry.id, step.detail));
                }
                notes.push(format!(
                    "{} adapter: {:?} — {}",
                    entry.id, resp.status, resp.message
                ));
            }
            Err(e) => notes.push(format!("{} adapter failed: {e}", entry.id)),
        }
    }
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

/// Best-effort absolute path (resolves symlinks when the path exists).
fn absolute(path: &Path) -> PathBuf {
    std::fs::canonicalize(path)
        .or_else(|_| std::path::absolute(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

/// Post-build health check (ADR-0013): always verifies the hardened artifact is
/// non-empty and (for native formats) still carries the source's magic; then runs
/// the opt-in `[verify]` command and `node --check` when configured. Any failure
/// fails the build so a broken artifact is not shipped.
fn post_build_check(
    src: &Path,
    dest: &Path,
    kind: TargetKind,
    out: &Path,
    verify: &direa_core::Verify,
) -> Result<()> {
    if file_or_dir_size(dest).unwrap_or(0) == 0 {
        return Err(refused(
            "post_build_empty",
            format!(
                "post-build check failed: hardened artifact {} is empty",
                dest.display()
            ),
        ));
    }
    if matches!(
        kind,
        TargetKind::NativeElf | TargetKind::NativeMachO | TargetKind::NativePe | TargetKind::Go
    ) && !same_magic(src, dest)
    {
        return Err(refused(
            "post_build_magic",
            format!(
                "post-build check failed: {} no longer carries the source executable magic",
                dest.display()
            ),
        ));
    }
    if let Some(command) = &verify.command {
        run_check_command(command, dest, out)?;
    }
    if verify.js_syntax && matches!(kind, TargetKind::JavaScript | TargetKind::ElectronAsar) {
        check_js_syntax(dest)?;
    }
    Ok(())
}

fn head(path: &Path) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut buf = [0u8; 4];
    let mut f = std::fs::File::open(path).ok()?;
    let n = f.read(&mut buf).ok()?;
    Some(buf[..n].to_vec())
}

fn same_magic(src: &Path, dest: &Path) -> bool {
    match (head(src), head(dest)) {
        (Some(a), Some(b)) => {
            if a.starts_with(b"MZ") {
                b.starts_with(b"MZ")
            } else {
                a == b
            }
        }
        _ => true,
    }
}

fn run_check_command(command: &str, artifact: &Path, out: &Path) -> Result<()> {
    let expanded = command
        .replace("{artifact}", &artifact.display().to_string())
        .replace("{out}", &out.display().to_string());
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(&expanded);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c").arg(&expanded);
        c
    };
    let output = cmd
        .output()
        .with_context(|| format!("running post-build command: {expanded}"))?;
    if !output.status.success() {
        return Err(refused(
            "verify_command_failed",
            format!(
                "post-build command failed ({}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    Ok(())
}

fn check_js_syntax(dest: &Path) -> Result<()> {
    let Some(node) = direa_core::which("node") else {
        // Documented: skipped when node is unavailable.
        return Ok(());
    };
    let mut files = Vec::new();
    collect_js(dest, &mut files);
    for file in files {
        let output = Command::new(&node)
            .arg("--check")
            .arg(&file)
            .output()
            .with_context(|| format!("running node --check on {}", file.display()))?;
        if !output.status.success() {
            return Err(refused(
                "js_syntax_failed",
                format!(
                    "post-build JS syntax check failed for {}: {}",
                    file.display(),
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            ));
        }
    }
    Ok(())
}

fn collect_js(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                collect_js(&entry.path(), out);
            }
        }
    } else if path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e, "js" | "mjs" | "cjs"))
        .unwrap_or(false)
    {
        out.push(path.to_path_buf());
    }
}

/// Refuse output that would overwrite the source artifact or be written inside
/// the source directory. This is the guard that keeps `build` from damaging the
/// user's codebase (ADR-0012, K-017).
fn guard_source_and_output(src: &Path, dest: &Path) -> Result<()> {
    let a = absolute(src);
    let d = absolute(dest);
    if d == a {
        return Err(refused(
            "output_over_source",
            format!(
                "refusing to write output over the source artifact ({}); choose an --out outside the source",
                src.display()
            ),
        ));
    }
    if a.is_dir() && d.starts_with(&a) {
        return Err(refused(
            "output_inside_source",
            format!(
                "refusing to write output inside the source directory ({}); choose an --out outside it",
                src.display()
            ),
        ));
    }
    Ok(())
}

/// The safe JS transforms shared by single files and asar members: source-map
/// and comment strip, the authorization notice, canaries and the embedded guard.
/// Decoys and tarpits are intentionally excluded here (they are applied once, at
/// the artifact level, not per embedded file).
fn harden_js_text(
    text: &str,
    policy: &ResolvedPolicy,
    notice: &str,
    canaries: &[String],
) -> String {
    let (body, _comments, _maps) = transform_js(text, policy);
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
    if policy.anti_instrumentation {
        out.push('\n');
        out.push_str(direa_core::guard::JS_GUARD_SOURCE);
    }
    out
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
    if policy.anti_instrumentation {
        out.push('\n');
        out.push_str(direa_core::guard::JS_GUARD_SOURCE);
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
            if policy.anti_instrumentation {
                out.push('\n');
                out.push_str(direa_core::guard::JS_GUARD_SOURCE);
            }
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
        stats.injected_string_runs += token_runs(padding.as_bytes());
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
        stats.injected_string_runs = token_runs(&data[data.len() - stats.padding_bytes as usize..]);
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
        for payload in tp.custom_payloads.iter().take(MAX_CUSTOM_PAYLOADS) {
            let bounded: String = payload.chars().take(MAX_CUSTOM_PAYLOAD_LEN).collect();
            out.push_str(&format!("/* {} */\n", sanitize_comment(&bounded)));
        }
        let (padding, bytes) = generate_padding(rng, tp.padding_kb, tp.max_total_kb, notice);
        out.push_str(&padding);
        stats.padding_bytes += bytes;
    }

    // Opt-in deceptive analyst notes (bounded; see the canary module).
    if policy.agent_decoys {
        let seed = rng.next_u64();
        out.push_str(&direa_core::canary::decoy_analysis(seed, 40));
    }

    stats.injected_bytes = before(&out);
    stats.injected_string_runs = token_runs(out.as_bytes());
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
    fn external_adapter_protocol_round_trips() {
        use direa_core::{AdapterOp, AdapterRequest, Status, SubprocessAdapter};
        let dir = std::env::temp_dir().join(format!("direa-js-adapter-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let target = dir.join("app.js");
        std::fs::write(&target, "var x = 1;\n").unwrap();
        let stub = dir.join("stub-adapter.sh");
        let script = r#"#!/bin/sh
input=$(cat)
target=$(printf '%s' "$input" | sed -n 's/.*"target":"\([^"]*\)".*/\1/p')
printf '\n/*stub-applied*/\n' >> "$target"
printf '{"adapter":"js","status":"ok","steps":[{"adapter":"js","action":"obfuscated","detail":"stub"}],"findings":[],"metrics":{},"message":"stub"}'
"#;
        std::fs::write(&stub, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let entry = AdapterEntry {
            id: "js".into(),
            command: stub.to_string_lossy().into_owned(),
            args: vec![],
            kinds: vec![TargetKind::JavaScript],
            builtin: false,
        };
        let request = AdapterRequest {
            protocol: direa_core::adapter::proto_version(),
            op: AdapterOp::Apply,
            target: target.clone(),
            kind: TargetKind::JavaScript,
            policy: direa_core::Profile::Low.resolve(),
            options: serde_json::json!({}),
        };
        let resp = SubprocessAdapter::new(entry).run(&request).unwrap();
        assert_eq!(resp.status, Status::Ok);
        assert_eq!(resp.steps.len(), 1);
        let out = std::fs::read_to_string(&target).unwrap();
        assert!(
            out.contains("stub-applied"),
            "adapter must transform the target"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn post_build_check_fails_on_bad_command() {
        let dir = std::env::temp_dir().join(format!("direa-postcheck-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("app.js");
        std::fs::write(&f, "var x = 1;\n").unwrap();

        let bad = direa_core::Verify {
            command: Some("exit 1".into()),
            js_syntax: false,
        };
        assert!(post_build_check(&f, &f, TargetKind::JavaScript, &dir, &bad).is_err());

        let ok = direa_core::Verify {
            command: Some("exit 0".into()),
            js_syntax: false,
        };
        assert!(post_build_check(&f, &f, TargetKind::JavaScript, &dir, &ok).is_ok());

        let empty = dir.join("empty");
        std::fs::write(&empty, b"").unwrap();
        assert!(post_build_check(
            &f,
            &empty,
            TargetKind::JavaScript,
            &dir,
            &Default::default()
        )
        .is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn js_guard_is_injected_when_anti_instrumentation() {
        let dir = std::env::temp_dir().join(format!("direa-guard-embed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("app.js");
        std::fs::write(&src, "var x = 1;\n").unwrap();
        let rz = Randomization {
            decoy_functions: 0,
            decoy_strings: 0,
            depth_bomb: 0,
            ..Default::default()
        };
        let tp = Tarpit {
            enabled: false,
            ..Default::default()
        };

        let high = direa_core::Profile::High.resolve();
        let dest_high = dir.join("high.js");
        harden_text_file(
            &src,
            &dest_high,
            &high,
            "n",
            &[],
            &rz,
            &tp,
            &mut Rng::new(1),
        )
        .unwrap();
        assert!(std::fs::read_to_string(&dest_high)
            .unwrap()
            .contains("__DIREA_GUARD__"));

        let low = direa_core::Profile::Low.resolve();
        let dest_low = dir.join("low.js");
        harden_text_file(&src, &dest_low, &low, "n", &[], &rz, &tp, &mut Rng::new(1)).unwrap();
        assert!(!std::fs::read_to_string(&dest_low)
            .unwrap()
            .contains("__DIREA_GUARD__"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn refuses_output_over_source() {
        let dir = std::env::temp_dir().join(format!("direa-guard-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("app.js");
        std::fs::write(&src, "var x = 1;\n").unwrap();
        // dest == src
        assert!(guard_source_and_output(&src, &src).is_err());
        // dest inside the source directory
        let nested = dir.join("nested").join("app.js");
        assert!(guard_source_and_output(&dir, &nested).is_err());
        // dest elsewhere is fine
        let elsewhere = dir.join("out").join("app.js");
        assert!(guard_source_and_output(&src, &elsewhere).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn asar_file_is_copied_not_decoded_as_utf8() {
        // A binary .asar must not be routed through the UTF-8 text path (B-043).
        let dir = std::env::temp_dir().join(format!("direa-asar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("app.asar");
        std::fs::write(&src, b"\x04\x00\x00\x00\xff\xfe binary archive").unwrap();

        let manifest = Manifest {
            targets: vec![direa_core::TargetSpec {
                path: src.clone(),
                kind: None,
                adapters: vec![],
            }],
            ..Default::default()
        };
        let out = dir.join("out");
        let res = build(&manifest, &out, false, Some(1), false)
            .expect("a single-file .asar must build without a UTF-8 error");
        assert_eq!(res.targets.len(), 1);
        assert!(
            res.targets[0].actions.iter().any(|a| a.contains("asar")),
            "the .asar branch must handle the archive"
        );
        assert!(out.join("app.asar").exists(), "archive must be written");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn valid_asar_is_hardened_in_place() {
        // Build a minimal valid asar (header pickle + data) with one JS file that
        // carries a source-map comment.
        let js = "var x=1;\n//# sourceMappingURL=x.js.map\n";
        let root = serde_json::json!({"files":{"index.js":{"size":js.len(),"offset":"0"}}});
        let json = serde_json::to_vec(&root).unwrap();
        let pad = (4 - json.len() % 4) % 4;
        let payload_len = 4 + json.len() + pad;
        let header_buf_len = 4 + payload_len;
        let mut archive = Vec::new();
        archive.extend_from_slice(&4u32.to_le_bytes());
        archive.extend_from_slice(&(header_buf_len as u32).to_le_bytes());
        archive.extend_from_slice(&(payload_len as u32).to_le_bytes());
        archive.extend_from_slice(&(json.len() as u32).to_le_bytes());
        archive.extend_from_slice(&json);
        archive.resize(archive.len() + pad, 0);
        archive.extend_from_slice(js.as_bytes());

        let dir = std::env::temp_dir().join(format!("direa-asar-valid-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("app.asar");
        std::fs::write(&src, &archive).unwrap();

        let manifest = Manifest {
            targets: vec![direa_core::TargetSpec {
                path: src.clone(),
                kind: None,
                adapters: vec![],
            }],
            ..Default::default()
        };
        let out = dir.join("out");
        let res = build(&manifest, &out, false, Some(1), false).unwrap();
        assert!(
            res.targets[0]
                .actions
                .iter()
                .any(|a| a.contains("asar: rewrote")),
            "a valid asar must take the rewrite path: {:?}",
            res.targets[0].actions
        );
        let hardened = std::fs::read(out.join("app.asar")).unwrap();
        assert_ne!(hardened, archive, "archive must be rewritten");
        assert!(
            !hardened.windows(17).any(|w| w == b"sourceMappingURL="),
            "embedded source-map reference must be stripped"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn jar_is_copied_and_defers_to_adapter() {
        // A `.jar` must be detected (not Unknown) and copied for the jvm adapter.
        let dir = std::env::temp_dir().join(format!("direa-jar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("app.jar");
        std::fs::write(&src, b"PK\x03\x04 not a real jar").unwrap();

        let manifest = Manifest {
            targets: vec![direa_core::TargetSpec {
                path: src.clone(),
                kind: None,
                adapters: vec![],
            }],
            ..Default::default()
        };
        let out = dir.join("out");
        let res = build(&manifest, &out, false, Some(1), false).unwrap();
        assert_eq!(res.targets.len(), 1);
        assert_eq!(res.targets[0].kind, "java-jar");
        assert!(res.targets[0].actions.iter().any(|a| a.contains("jar")));
        assert!(out.join("app.jar").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn custom_payloads_are_bounded_and_sanitized() {
        let policy = direa_core::Profile::Paranoid.resolve();
        let rz = Randomization {
            decoy_functions: 0,
            decoy_strings: 0,
            depth_bomb: 0,
            ..Default::default()
        };
        let big = "x".repeat(MAX_CUSTOM_PAYLOAD_LEN + 50);
        let tp = Tarpit {
            enabled: true,
            padding_kb: 0,
            honeypots: 0,
            custom_payloads: vec!["a*/b".into(), big],
            ..Default::default()
        };
        let (out, _) = generate_js_bundle(&mut Rng::new(1), &policy, &rz, &tp, "n");
        assert!(out.contains("a* /b"), "payload comment must be sanitized");
        assert!(
            !out.contains(&"x".repeat(MAX_CUSTOM_PAYLOAD_LEN + 1)),
            "payload must be truncated"
        );
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
