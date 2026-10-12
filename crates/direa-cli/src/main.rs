use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

use direa_core::manifest::MANIFEST_FILE;
use direa_core::{plan_manifest, Manifest, Profile};

mod build;
mod ui;

use ui::Ui;

#[derive(Parser, Debug)]
#[command(
    name = "die-a-rea",
    about = "Harden software against automated / LLM reverse engineering and measure the result.",
    version,
    propagate_version = true
)]
struct Cli {
    /// Path to the manifest file.
    #[arg(long, global = true, default_value = MANIFEST_FILE)]
    manifest: PathBuf,

    /// Increase verbosity (repeatable): -v phases/timing, -vv transform trace.
    #[arg(long, short, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Suppress non-error output.
    #[arg(long, short, global = true)]
    quiet: bool,

    /// Format for error output.
    #[arg(long, global = true, value_enum, default_value_t = ErrorFormat::Human)]
    error_format: ErrorFormat,

    #[command(subcommand)]
    command: Command,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum ErrorFormat {
    Human,
    Json,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Create a starter die-a-rea.toml in the current directory.
    Init {
        /// Project name written into the manifest.
        #[arg(long)]
        name: Option<String>,
        /// One or more target paths to pre-fill.
        targets: Vec<String>,
        /// Overwrite an existing manifest.
        #[arg(long)]
        force: bool,
    },

    /// List the built-in protection profiles.
    Profiles,

    /// Report toolchain, adapter and target readiness.
    Doctor {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },

    /// Run the agent-defense detector gauntlet (CI-friendly; exits non-zero on failure).
    CanarySelftest {
        /// Optional external detector command (stdin = text, exit 0 = injection).
        #[arg(long)]
        detector: Option<String>,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },

    /// Show the hardening plan for the manifest (no side effects).
    Plan {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },

    /// Produce hardened artifacts.
    Build {
        /// Output directory for hardened artifacts.
        #[arg(long, default_value = "dist-hardened")]
        out: PathBuf,
        /// Show what would be done without writing anything.
        #[arg(long)]
        dry_run: bool,
        /// Override the randomization seed (reproducible decoy layout).
        #[arg(long)]
        seed: Option<u64>,
        /// Overwrite an existing output directory die-a-rea did not create.
        #[arg(long)]
        force: bool,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },

    /// Measure the reverse-engineering Resistance Score (RRS).
    Verify {
        /// The unhardened reference artifact.
        #[arg(long)]
        original: PathBuf,
        /// The hardened artifact (defaults to the build output).
        #[arg(long)]
        hardened: Option<PathBuf>,
        /// Also run REA as an external adversary when available.
        #[arg(long)]
        with_rea: bool,
        /// REA command prefix (without the target path).
        #[arg(long, num_args = 1.., default_values_t = default_rea_command())]
        rea_command: Vec<String>,
        /// Build-metadata sidecar produced by `build` (defaults beside the
        /// hardened artifact).
        #[arg(long)]
        build_meta: Option<PathBuf>,
        /// Also measure the runtime-instrumentation axis against this environment.
        #[arg(long)]
        with_runtime: bool,
        /// External decompiler command for the control-flow axis ({artifact} substituted).
        #[arg(long)]
        decompiler: Option<String>,
        /// Write the JSON report to this path.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Emit machine-readable JSON to stdout.
        #[arg(long)]
        json: bool,
    },
}

fn default_rea_command() -> Vec<String> {
    vec![
        "npx".into(),
        "-y".into(),
        "rea-agents@latest".into(),
        "analyze-javascript-application".into(),
    ]
}

fn load_manifest(path: &PathBuf) -> Result<Manifest> {
    if !path.exists() {
        return Err(direa_core::DireaError::Refused {
            code: "manifest_missing",
            message: format!(
                "manifest not found at {} - run `die-a-rea init` first",
                path.display()
            ),
        }
        .into());
    }
    Manifest::load(path).with_context(|| format!("loading {}", path.display()))
}

fn main() {
    let cli = Cli::parse();
    let error_format = cli.error_format;
    if let Err(err) = run(cli) {
        report_error(&err, error_format);
        std::process::exit(1);
    }
}

/// Print an error with a stable code, in human or JSON form.
fn report_error(err: &anyhow::Error, format: ErrorFormat) {
    let code = err
        .chain()
        .find_map(|cause| cause.downcast_ref::<direa_core::DireaError>())
        .map(|e| e.code())
        .unwrap_or("error");
    match format {
        ErrorFormat::Human => {
            eprintln!("error[{code}]: {err}");
            for cause in err.chain().skip(1) {
                eprintln!("  caused by: {cause}");
            }
        }
        ErrorFormat::Json => {
            let causes: Vec<String> = err.chain().skip(1).map(|c| c.to_string()).collect();
            let envelope = serde_json::json!({
                "error": { "code": code, "message": err.to_string(), "causes": causes }
            });
            eprintln!("{}", envelope);
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    let ui = Ui::new(cli.verbose, cli.quiet);
    match cli.command {
        Command::Init {
            name,
            targets,
            force,
        } => cmd_init(&ui, &cli.manifest, name, targets, force),
        Command::Profiles => cmd_profiles(&ui),
        Command::Doctor { json } => cmd_doctor(&ui, &cli.manifest, json),
        Command::CanarySelftest { detector, json } => cmd_canary_selftest(&ui, detector, json),
        Command::Plan { json } => cmd_plan(&ui, &cli.manifest, json),
        Command::Build {
            out,
            dry_run,
            seed,
            force,
            json,
        } => cmd_build(&ui, &cli.manifest, out, dry_run, seed, force, json),
        Command::Verify {
            original,
            hardened,
            with_rea,
            rea_command,
            build_meta,
            with_runtime,
            decompiler,
            out,
            json,
        } => cmd_verify(
            &ui,
            cli.manifest.clone(),
            original,
            hardened,
            with_rea,
            rea_command,
            build_meta,
            with_runtime,
            decompiler,
            out,
            json,
        ),
    }
}

fn cmd_init(
    ui: &Ui,
    path: &PathBuf,
    name: Option<String>,
    targets: Vec<String>,
    force: bool,
) -> Result<()> {
    if path.exists() && !force {
        bail!(
            "{} already exists (use --force to overwrite)",
            path.display()
        );
    }
    let name = name.unwrap_or_else(|| {
        std::env::current_dir()
            .ok()
            .and_then(|d| d.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| "app".to_string())
    });
    let content = Manifest::starter(&name, &targets);
    std::fs::write(path, content).with_context(|| format!("writing {}", path.display()))?;
    ui.info(2, format!("starter manifest for project '{name}'"));
    ui.out(format!("wrote {}", path.display()));
    Ok(())
}

fn cmd_profiles(ui: &Ui) -> Result<()> {
    ui.out(format!(
        "{:<10} {:<10} {:<10} {:<12} {:<10} {:<10}",
        "profile", "symbols", "strings", "controlflow", "runtime", "agent"
    ));
    for profile in [
        Profile::Low,
        Profile::Medium,
        Profile::High,
        Profile::Paranoid,
    ] {
        let p = profile.resolve();
        ui.out(format!(
            "{:<10} {:<10} {:<10} {:<12} {:<10} {:<10}",
            profile.as_str(),
            p.strip_symbols,
            p.encrypt_strings,
            p.obfuscate_control_flow,
            p.anti_instrumentation,
            if p.agent_decoys {
                "decoys"
            } else if p.agent_canaries {
                "canary"
            } else {
                "notice"
            },
        ));
    }
    Ok(())
}

#[derive(serde::Serialize)]
struct DoctorCheck {
    name: String,
    detail: String,
    ok: bool,
}

#[derive(serde::Serialize)]
struct DoctorReport {
    platform: String,
    checks: Vec<DoctorCheck>,
}

fn cmd_doctor(ui: &Ui, manifest_path: &Path, json: bool) -> Result<()> {
    let mut checks = vec![DoctorCheck {
        name: "manifest".into(),
        detail: manifest_path.display().to_string(),
        ok: manifest_path.exists(),
    }];

    for entry in direa_core::planner::default_registry()
        .iter()
        .filter(|e| !e.builtin)
    {
        let key = direa_core::adapter::env_key(&entry.id);
        let via_env = std::env::var(&key).ok().filter(|v| !v.trim().is_empty());
        let via_path = direa_core::which(&entry.command);
        let ok = via_env.is_some() || via_path.is_some();
        let detail = if let Some(v) = via_env {
            format!("{key}={v}")
        } else if let Some(p) = via_path {
            p.display().to_string()
        } else {
            format!("set {key} or add `{}` to PATH", entry.command)
        };
        checks.push(DoctorCheck {
            name: format!("adapter:{}", entry.id),
            detail,
            ok,
        });
    }

    for tool in [
        "sh",
        "node",
        "npm",
        "javascript-obfuscator",
        "go",
        "garble",
        "clang",
        "dotnet",
        "r8",
        "proguard",
        "nm",
        "objdump",
        "llvm-nm",
    ] {
        let ok = direa_core::which(tool).is_some();
        checks.push(DoctorCheck {
            name: format!("tool:{tool}"),
            detail: if ok {
                "found".into()
            } else {
                "absent (optional)".into()
            },
            ok,
        });
    }

    let report = DoctorReport {
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        checks,
    };
    if json {
        ui.out(serde_json::to_string_pretty(&report)?);
    } else {
        ui.out(format!("platform: {}", report.platform));
        for c in &report.checks {
            ui.out(format!(
                "  {} {:<24} {}",
                if c.ok { "ok" } else { "--" },
                c.name,
                c.detail
            ));
        }
        let ready = report.checks.iter().filter(|c| c.ok).count();
        ui.out(format!("{ready}/{} checks ok", report.checks.len()));
    }
    Ok(())
}

/// Run an external detector command over `text`; exit 0 means "injection".
fn run_detector(command: &str, text: &str) -> bool {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = match Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }
    child.wait().map(|s| s.success()).unwrap_or(false)
}

fn cmd_canary_selftest(ui: &Ui, detector: Option<String>, json: bool) -> Result<()> {
    let report = match &detector {
        Some(cmd) => {
            let hook = |s: &str| run_detector(cmd, s);
            direa_core::canary::gauntlet_with(Some(&hook))
        }
        None => direa_core::canary::gauntlet(),
    };
    if json {
        ui.out(serde_json::to_string_pretty(&report)?);
    } else {
        ui.out(format!(
            "adversarial detected: {}/{}",
            report.adversarial_detected, report.adversarial_total
        ));
        ui.out(format!(
            "own payloads flagged: {}/{}",
            report.payload_flagged, report.payload_total
        ));
        ui.out(if report.passed { "PASS" } else { "FAIL" });
    }
    if !report.passed {
        bail!("canary gauntlet failed: the detector missed adversarial samples");
    }
    Ok(())
}

fn cmd_plan(ui: &Ui, manifest_path: &PathBuf, json: bool) -> Result<()> {
    let manifest = load_manifest(manifest_path)?;
    let registry = direa_core::planner::default_registry();
    let plans = ui.timed(1, "plan", || plan_manifest(&manifest, &registry));

    if json {
        ui.out(serde_json::to_string_pretty(&plans)?);
        return Ok(());
    }

    if plans.is_empty() {
        ui.out(format!(
            "no targets configured in {}",
            manifest_path.display()
        ));
        return Ok(());
    }
    for plan in &plans {
        ui.out(format!("target: {} ({})", plan.target, plan.kind.as_str()));
        for step in &plan.steps {
            ui.out(format!(
                "  - {:<14} {:<18} {}",
                step.adapter, step.action, step.detail
            ));
        }
        if !plan.missing.is_empty() {
            ui.out(format!(
                "  ! missing adapters (install for deeper protection): {}",
                plan.missing.join(", ")
            ));
        }
    }
    Ok(())
}

fn cmd_build(
    ui: &Ui,
    manifest_path: &PathBuf,
    out: PathBuf,
    dry_run: bool,
    seed: Option<u64>,
    force: bool,
    json: bool,
) -> Result<()> {
    let manifest = load_manifest(manifest_path)?;
    let result = ui.timed(1, "build", || {
        build::build(&manifest, &out, dry_run, seed, force)
    })?;

    // Transform trace at -vv.
    if !json && ui.verbosity >= 2 {
        for target in &result.targets {
            ui.info(
                2,
                format_args!("{} ({}) -> {}", target.target, target.kind, target.output),
            );
            for action in &target.actions {
                ui.info(2, format_args!("  - {action}"));
            }
        }
    }

    if json {
        ui.out(serde_json::to_string_pretty(&result)?);
    } else {
        for note in &result.notes {
            ui.out(note);
        }
        if dry_run {
            ui.out("dry run: no files written");
        } else {
            ui.out(format!("hardened output: {}", result.output_dir.display()));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_verify(
    ui: &Ui,
    manifest_path: PathBuf,
    original: PathBuf,
    hardened: Option<PathBuf>,
    with_rea: bool,
    rea_command: Vec<String>,
    build_meta: Option<PathBuf>,
    with_runtime: bool,
    decompiler: Option<String>,
    out: Option<PathBuf>,
    json: bool,
) -> Result<()> {
    let hardened = hardened.unwrap_or_else(|| {
        let name = original
            .file_name()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("artifact"));
        PathBuf::from("dist-hardened").join(name)
    });
    let mut opts = direa_verify::VerifyOptions::new(&original, &hardened);
    opts.with_rea = with_rea;
    opts.rea_command = rea_command;
    opts.build_meta = build_meta;
    opts.with_runtime = with_runtime;
    opts.decompiler = decompiler;
    // Honor `[runtime] instrumentation_signatures` when a manifest is present.
    if let Ok(manifest) = Manifest::load(&manifest_path) {
        opts.signatures = manifest.runtime.instrumentation_signatures;
    }

    let run = ui.timed(1, "verify", || {
        direa_verify::run(&opts).map_err(|e| anyhow::anyhow!(e))
    })?;
    let report = &run.report;

    if json {
        ui.out(serde_json::to_string_pretty(report)?);
    } else {
        ui.out(format!("target: {}", report.target));
        ui.out(format!(
            "RRS: {}/100 (grade {})",
            report.rrs,
            report.grade.as_str()
        ));
        for m in &report.measurements {
            let delta = m.delta();
            ui.out(format!(
                "  {:<30} exposure {:.2} -> {:.2}  (delta {:+.2})",
                m.axis.as_str(),
                m.baseline,
                m.hardened,
                delta
            ));
        }
        if !report.notes.is_empty() {
            ui.out("\nnotes:");
            for note in &report.notes {
                ui.out(format!("  - {note}"));
            }
        }
    }

    if let Some(path) = out {
        std::fs::write(&path, serde_json::to_string_pretty(report)?)
            .with_context(|| format!("writing {}", path.display()))?;
        if !json {
            ui.out(format!("\nwrote report to {}", path.display()));
        }
    }
    Ok(())
}
