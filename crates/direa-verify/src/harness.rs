use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use direa_core::{Axis, AxisMeasurement, BuildMetadata, ResistanceReport, TargetMetadata};
use serde::{Deserialize, Serialize};

use crate::probes::{default_probes, rea_target, Probe, ProbeKind};

/// Options controlling a verification run.
#[derive(Debug, Clone)]
pub struct VerifyOptions {
    /// The unhardened reference artifact.
    pub original: PathBuf,
    /// The hardened artifact produced by `die-a-rea build`.
    pub hardened: PathBuf,
    /// Whether to also run REA, when available, as an external adversary.
    pub with_rea: bool,
    /// Command prefix for the REA adversary (without the target argument).
    pub rea_command: Vec<String>,
    /// Timeout for the REA adversary in seconds.
    pub rea_timeout_secs: u64,
    /// Explicit build-metadata sidecar; defaults to the convention next to the
    /// hardened artifact when present.
    pub build_meta: Option<PathBuf>,
    /// Also measure the runtime-instrumentation axis (checks the current
    /// environment for a debugger / Frida / inspector).
    pub with_runtime: bool,
    /// Extra instrumentation signatures (e.g. `[runtime]
    /// instrumentation_signatures`). Empty uses the built-in defaults.
    pub signatures: Vec<String>,
    /// External decompiler/disassembler command for the control-flow axis
    /// (`{artifact}` substituted). When absent the axis is skipped (ADR-0017).
    pub decompiler: Option<String>,
}

impl VerifyOptions {
    pub fn new(original: impl Into<PathBuf>, hardened: impl Into<PathBuf>) -> Self {
        VerifyOptions {
            original: original.into(),
            hardened: hardened.into(),
            with_rea: false,
            rea_command: vec![
                "npx".into(),
                "-y".into(),
                "rea-agents@latest".into(),
                "analyze-javascript-application".into(),
            ],
            rea_timeout_secs: 120,
            build_meta: None,
            with_runtime: false,
            signatures: Vec::new(),
            decompiler: None,
        }
    }
}

/// Result of the optional REA adversary run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReaObservation {
    pub available: bool,
    pub succeeded: bool,
    pub canary_hits: Vec<String>,
    pub detail: String,
}

/// Full verification run.
#[derive(Debug, Clone)]
pub struct VerifyRun {
    pub report: ResistanceReport,
    pub rea: Option<ReaObservation>,
}

/// Run all probes against both artifacts and produce an RRS report.
pub fn run(opts: &VerifyOptions) -> Result<VerifyRun, String> {
    if !opts.original.exists() {
        return Err(format!("original not found: {}", opts.original.display()));
    }
    if !opts.hardened.exists() {
        return Err(format!("hardened not found: {}", opts.hardened.display()));
    }
    let meta = load_build_meta(opts);
    let mut probes = default_probes();
    if let Some(command) = &opts.decompiler {
        probes.push(Box::new(crate::probes::ExternalControlFlowProbe {
            command: command.clone(),
        }));
    }
    let (mut measurements, mut notes) =
        measure(&opts.original, &opts.hardened, &probes, meta.as_ref());
    if meta.is_none() {
        notes.push(
            "no build metadata found; analysis-noise axis omitted and decoy strings not discounted"
                .to_string(),
        );
    }

    if opts.with_runtime {
        let report = direa_core::guard::detect_with(&opts.signatures);
        match crate::runtime::runtime_measurement(&report) {
            Some(m) => {
                notes.push(format!(
                    "runtime-instrumentation: instrumented environment detected ({})",
                    report.findings.join(", ")
                ));
                measurements.push(m);
            }
            None => notes.push(
                "runtime-instrumentation: no instrumentation detected; axis skipped (unknown)"
                    .to_string(),
            ),
        }
    }

    let mut rea = None;
    if opts.with_rea {
        match run_rea(opts) {
            Some(obs) => {
                notes.push(format!(
                    "REA adversary: available={} succeeded={} canaries_seen={} ({})",
                    obs.available,
                    obs.succeeded,
                    obs.canary_hits.len(),
                    obs.detail
                ));
                rea = Some(obs);
            }
            None => {
                notes.push("REA adversary skipped (hardened target does not exist)".to_string())
            }
        }
    }

    let report = ResistanceReport::new(opts.original.display().to_string(), measurements, notes);
    Ok(VerifyRun { report, rea })
}

/// Load the build metadata sidecar for the hardened artifact, if any.
fn load_build_meta(opts: &VerifyOptions) -> Option<TargetMetadata> {
    let path = opts
        .build_meta
        .clone()
        .unwrap_or_else(|| BuildMetadata::sidecar_for(&opts.hardened));
    if !path.exists() {
        return None;
    }
    let meta = BuildMetadata::load(&path).ok()?;
    meta.find(&opts.hardened).cloned()
}

/// Measure every probe on both artifacts and normalise to exposures.
///
/// When build metadata is available, injected decoy strings are discounted from
/// the hardened string count (so decoys are not mistaken for recovered real
/// content) and an analysis-noise axis is added.
pub fn measure(
    original: &Path,
    hardened: &Path,
    probes: &[Box<dyn Probe>],
    meta: Option<&TargetMetadata>,
) -> (Vec<AxisMeasurement>, Vec<String>) {
    let mut measurements = Vec::new();
    let mut notes = Vec::new();
    for probe in probes {
        let base = probe.measure(original);
        let hard = probe.measure(hardened);
        match (base, hard) {
            (Ok(b), Ok(h)) => {
                // The string axis is a signal-to-noise measure: an analyst can
                // always read strings, so resistance comes from diluting real
                // strings with decoys.
                if probe.name() == "strings" {
                    // Verbatim survival: how many of the baseline's tokens are
                    // still readable in the hardened artifact. This rewards
                    // removal (native-post) and encoding (JS obfuscation) without
                    // any heuristics; dilution is scored by analysis-noise
                    // (ADR-0011, K-016).
                    let base = token_set(original);
                    let hard = token_set(hardened);
                    let be = if base.is_empty() { 0.0 } else { 1.0 };
                    let leaked = base.intersection(&hard).count() as f64;
                    let he = if base.is_empty() {
                        0.0
                    } else {
                        (leaked / base.len() as f64).clamp(0.0, 1.0)
                    };
                    measurements.push(AxisMeasurement::new(probe.axis(), be, he));
                    notes.push(format!(
                        "{}: {:.0} of {:.0} baseline tokens still verbatim (exposure {:.2} -> {:.2})",
                        probe.name(),
                        leaked,
                        base.len(),
                        be,
                        he
                    ));
                } else if b.raw <= 0.0 {
                    // Nothing to measure against: do not reward an inapplicable
                    // axis as if it were fully resisted.
                    notes.push(format!(
                        "{}: skipped (no baseline signal; not applicable to this target)",
                        probe.name()
                    ));
                } else {
                    let (be, he) = exposures(probe.kind(), b.raw, h.raw);
                    measurements.push(AxisMeasurement::new(probe.axis(), be, he));
                    notes.push(format!(
                        "{}: baseline={} -> hardened={} (exposure {:.2} -> {:.2})",
                        probe.name(),
                        b.detail,
                        h.detail,
                        be,
                        he
                    ));
                }
            }
            (Err(e), _) | (_, Err(e)) => {
                notes.push(format!("{}: skipped ({e})", probe.name()));
            }
        }
    }

    if let Some(meta) = meta {
        let noise = meta.noise_fraction();
        // Exposure semantics: baseline (no noise) is fully exposed; resistance
        // equals the injected noise fraction.
        measurements.push(AxisMeasurement::new(Axis::AnalysisNoise, 1.0, 1.0 - noise));
        notes.push(format!(
            "analysis-noise: {:.0}% injected ({} of {} bytes)",
            noise * 100.0,
            meta.injected.injected_bytes,
            meta.total_bytes
        ));
    }

    (measurements, notes)
}

/// Distinct identifier/literal tokens (>= 6 bytes) across a target's files.
fn token_set(path: &Path) -> BTreeSet<Vec<u8>> {
    let mut set = BTreeSet::new();
    if let Ok(blobs) = crate::probes::collect_bytes(path) {
        for bytes in &blobs {
            for token in direa_core::scan::token_runs(bytes, direa_core::scan::DEFAULT_MIN_TOKEN) {
                set.insert(token.to_vec());
            }
        }
    }
    set
}

/// Compute `(baseline_exposure, hardened_exposure)` in `[0, 1]`.
fn exposures(kind: ProbeKind, baseline: f64, hardened: f64) -> (f64, f64) {
    match kind {
        ProbeKind::Boolean => (baseline.clamp(0.0, 1.0), hardened.clamp(0.0, 1.0)),
        ProbeKind::Count => {
            if baseline <= 0.0 {
                (0.0, 0.0)
            } else {
                (1.0, (hardened / baseline).clamp(0.0, 1.0))
            }
        }
    }
}

/// Run REA as an external adversary against the hardened artifact.
fn run_rea(opts: &VerifyOptions) -> Option<ReaObservation> {
    let target = rea_target(&opts.hardened)?;
    if opts.rea_command.is_empty() {
        return None;
    }
    let (program, args) = opts.rea_command.split_first()?;

    let mut cmd = Command::new(program);
    cmd.args(args)
        .arg(&target)
        .arg("--json")
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return Some(ReaObservation {
                available: false,
                succeeded: false,
                canary_hits: Vec::new(),
                detail: format!("could not launch REA: {e}"),
            })
        }
    };

    let stdout = child.stdout.take();
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(mut o) = stdout {
            let _ = o.read_to_string(&mut s);
        }
        s
    });

    let deadline = Instant::now() + Duration::from_secs(opts.rea_timeout_secs.max(1));
    let mut timed_out = false;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    timed_out = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(_) => break,
        }
    }

    let output = reader.join().unwrap_or_default();
    let canary_hits = collect_canaries(&output);

    Some(ReaObservation {
        available: true,
        succeeded: !timed_out && !output.trim().is_empty(),
        canary_hits,
        detail: if timed_out {
            "timed out".to_string()
        } else if output.trim().is_empty() {
            "no output".to_string()
        } else {
            format!("{} bytes of Evidence", output.len())
        },
    })
}

/// Find die-a-rea canary tokens in an adversary's output.
fn collect_canaries(text: &str) -> Vec<String> {
    const PREFIX: &str = "DIREA-CANARY-";
    let mut hits = Vec::new();
    let mut rest = text;
    while let Some(pos) = rest.find(PREFIX) {
        let start = pos;
        let tail = &rest[start + PREFIX.len()..];
        let end = tail
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '-')
            .unwrap_or(tail.len());
        hits.push(rest[start..start + PREFIX.len() + end].to_string());
        rest = &rest[start + PREFIX.len() + end..];
    }
    hits.sort();
    hits.dedup();
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_tmp(name: &str, bytes: &[u8]) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("direa-harness-{name}"));
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(bytes).unwrap();
        p
    }

    #[test]
    fn counts_normalise_by_ratio() {
        assert_eq!(exposures(ProbeKind::Count, 100.0, 10.0), (1.0, 0.1));
        assert_eq!(exposures(ProbeKind::Count, 0.0, 5.0), (0.0, 0.0));
    }

    #[test]
    fn boolean_passthrough() {
        assert_eq!(exposures(ProbeKind::Boolean, 1.0, 0.0), (1.0, 0.0));
    }

    #[test]
    fn parses_canaries() {
        let hits = collect_canaries("blah DIREA-CANARY-abc123, and DIREA-CANARY-abc123 again");
        assert_eq!(hits, vec!["DIREA-CANARY-abc123".to_string()]);
    }

    #[test]
    fn rea_runs_against_any_artifact() {
        // A native ELF (not JS/asar) must still be handed to the configured REA.
        let target = write_tmp("rea.bin", b"\x7fELF\x02\x01\x01\x00padding");
        let mut opts = VerifyOptions::new(&target, &target);
        opts.with_rea = true;
        opts.rea_command = vec!["sh".into(), "-c".into(), "echo DIREA-CANARY-zzz".into()];
        let obs = run_rea(&opts).expect("a native target must be analyzable");
        assert!(obs.available && obs.succeeded);
        assert_eq!(obs.canary_hits, vec!["DIREA-CANARY-zzz".to_string()]);
        let _ = std::fs::remove_file(target);
    }

    #[test]
    fn rea_skips_a_missing_target() {
        let mut opts = VerifyOptions::new("direa-no-such-orig", "direa-no-such-hard");
        opts.with_rea = true;
        opts.rea_command = vec!["sh".into(), "-c".into(), "echo hi".into()];
        assert!(run_rea(&opts).is_none());
    }

    #[test]
    fn end_to_end_static() {
        let original = write_tmp(
            "orig.bin",
            b"\x7fELF\x00.debug_info\x00.symtab\x00this is a long string\x00and another string\x00",
        );
        let hardened = write_tmp("hard.bin", b"\x7fELF\x00\x01\x02\x03");
        let opts = VerifyOptions::new(&original, &hardened);
        let run = run(&opts).unwrap();
        assert!(run.report.rrs > 0);
        assert_eq!(
            run.report.grade,
            direa_core::Grade::from_score(run.report.rrs)
        );
        let _ = std::fs::remove_file(original);
        let _ = std::fs::remove_file(hardened);
    }
}
