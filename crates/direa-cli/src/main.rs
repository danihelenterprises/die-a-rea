use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use direa_core::manifest::MANIFEST_FILE;
use direa_core::{plan_manifest, Manifest, Profile};

mod build;

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

    #[command(subcommand)]
    command: Command,
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
        bail!(
            "manifest not found at {} - run `die-a-rea init` first",
            path.display()
        );
    }
    Manifest::load(path).with_context(|| format!("loading {}", path.display()))
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Init {
            name,
            targets,
            force,
        } => cmd_init(&cli.manifest, name, targets, force),
        Command::Profiles => cmd_profiles(),
        Command::CanarySelftest { detector, json } => cmd_canary_selftest(detector, json),
        Command::Plan { json } => cmd_plan(&cli.manifest, json),
        Command::Build {
            out,
            dry_run,
            seed,
            force,
            json,
        } => cmd_build(&cli.manifest, out, dry_run, seed, force, json),
        Command::Verify {
            original,
            hardened,
            with_rea,
            rea_command,
            build_meta,
            with_runtime,
            out,
            json,
        } => cmd_verify(
            cli.manifest.clone(),
            original,
            hardened,
            with_rea,
            rea_command,
            build_meta,
            with_runtime,
            out,
            json,
        ),
    }
}

fn cmd_init(path: &PathBuf, name: Option<String>, targets: Vec<String>, force: bool) -> Result<()> {
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
    println!("wrote {}", path.display());
    Ok(())
}

fn cmd_profiles() -> Result<()> {
    println!(
        "{:<10} {:<10} {:<10} {:<12} {:<10} {:<10}",
        "profile", "symbols", "strings", "controlflow", "runtime", "agent"
    );
    for profile in [
        Profile::Low,
        Profile::Medium,
        Profile::High,
        Profile::Paranoid,
    ] {
        let p = profile.resolve();
        println!(
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
        );
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

fn cmd_canary_selftest(detector: Option<String>, json: bool) -> Result<()> {
    let report = match &detector {
        Some(cmd) => {
            let hook = |s: &str| run_detector(cmd, s);
            direa_core::canary::gauntlet_with(Some(&hook))
        }
        None => direa_core::canary::gauntlet(),
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "adversarial detected: {}/{}",
            report.adversarial_detected, report.adversarial_total
        );
        println!(
            "own payloads flagged: {}/{}",
            report.payload_flagged, report.payload_total
        );
        println!("{}", if report.passed { "PASS" } else { "FAIL" });
    }
    if !report.passed {
        bail!("canary gauntlet failed: the detector missed adversarial samples");
    }
    Ok(())
}

fn cmd_plan(manifest_path: &PathBuf, json: bool) -> Result<()> {
    let manifest = load_manifest(manifest_path)?;
    let registry = direa_core::planner::default_registry();
    let plans = plan_manifest(&manifest, &registry);

    if json {
        println!("{}", serde_json::to_string_pretty(&plans)?);
        return Ok(());
    }

    if plans.is_empty() {
        println!("no targets configured in {}", manifest_path.display());
        return Ok(());
    }
    for plan in &plans {
        println!("target: {} ({})", plan.target, plan.kind.as_str());
        for step in &plan.steps {
            println!(
                "  - {:<14} {:<18} {}",
                step.adapter, step.action, step.detail
            );
        }
        if !plan.missing.is_empty() {
            println!(
                "  ! missing adapters (install for deeper protection): {}",
                plan.missing.join(", ")
            );
        }
    }
    Ok(())
}

fn cmd_build(
    manifest_path: &PathBuf,
    out: PathBuf,
    dry_run: bool,
    seed: Option<u64>,
    force: bool,
    json: bool,
) -> Result<()> {
    let manifest = load_manifest(manifest_path)?;
    let result = build::build(&manifest, &out, dry_run, seed, force)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        for note in &result.notes {
            println!("{note}");
        }
        if dry_run {
            println!("dry run: no files written");
        } else {
            println!("hardened output: {}", result.output_dir.display());
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_verify(
    manifest_path: PathBuf,
    original: PathBuf,
    hardened: Option<PathBuf>,
    with_rea: bool,
    rea_command: Vec<String>,
    build_meta: Option<PathBuf>,
    with_runtime: bool,
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
    // Honor `[runtime] instrumentation_signatures` when a manifest is present.
    if let Ok(manifest) = Manifest::load(&manifest_path) {
        opts.signatures = manifest.runtime.instrumentation_signatures;
    }

    let run = direa_verify::run(&opts).map_err(|e| anyhow::anyhow!(e))?;
    let report = &run.report;

    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        println!("target: {}", report.target);
        println!("RRS: {}/100 (grade {})", report.rrs, report.grade.as_str());
        for m in &report.measurements {
            let delta = m.delta();
            println!(
                "  {:<30} exposure {:.2} -> {:.2}  (delta {:+.2})",
                m.axis.as_str(),
                m.baseline,
                m.hardened,
                delta
            );
        }
        if !report.notes.is_empty() {
            println!("\nnotes:");
            for note in &report.notes {
                println!("  - {note}");
            }
        }
    }

    if let Some(path) = out {
        std::fs::write(&path, serde_json::to_string_pretty(report)?)
            .with_context(|| format!("writing {}", path.display()))?;
        if !json {
            println!("\nwrote report to {}", path.display());
        }
    }
    Ok(())
}
