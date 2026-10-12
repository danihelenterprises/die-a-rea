//! Reverse-engineering Resistance Score (RRS).
//!
//! RRS is a 0-100 measure of how hard a hardened artifact is for automated and
//! LLM-driven tooling to recover useful meaning from. Every axis is expressed
//! as an *exposure* in `[0, 1]`, where `1.0` means fully recovered by the
//! adversary and `0.0` means nothing recoverable. The score is the weighted
//! mean of `1 - hardened_exposure`, so higher is better.
//!
//! Baseline (pre-hardening) exposure is recorded for reporting and delta
//! display but does not contribute to the score itself.

use serde::{Deserialize, Serialize};

/// Measured axis of reverse-engineering resistance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Axis {
    /// Symbol / function name recovery (`nm`, ELF/Mach-O symbol tables).
    SymbolExposure,
    /// Human-readable string recovery (`strings`, decompiler strings view).
    StringExposure,
    /// Debug metadata / DWARF / PDB recovery.
    DebugMetadataExposure,
    /// Source-map recovery for JavaScript / Electron bundles.
    SourceMapExposure,
    /// Decompiler control-flow / pseudocode fidelity.
    ControlFlowExposure,
    /// Success of runtime instrumentation (debugger, Frida, CDP, V8 inspector).
    RuntimeInstrumentationExposure,
    /// Whether an LLM analysis agent was deceived or gated by the canary layer.
    AgentDeception,
    /// Share of injected decoy/tarpit content (randomization + padding).
    AnalysisNoise,
}

impl Axis {
    pub fn weight(self) -> f64 {
        match self {
            Axis::SymbolExposure => 1.0,
            Axis::StringExposure => 1.5,
            Axis::DebugMetadataExposure => 0.8,
            Axis::SourceMapExposure => 1.2,
            Axis::ControlFlowExposure => 1.5,
            Axis::RuntimeInstrumentationExposure => 1.0,
            Axis::AgentDeception => 2.0,
            Axis::AnalysisNoise => 1.5,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Axis::SymbolExposure => "symbol-exposure",
            Axis::StringExposure => "string-exposure",
            Axis::DebugMetadataExposure => "debug-metadata-exposure",
            Axis::SourceMapExposure => "source-map-exposure",
            Axis::ControlFlowExposure => "control-flow-exposure",
            Axis::RuntimeInstrumentationExposure => "runtime-instrumentation-exposure",
            Axis::AgentDeception => "agent-deception",
            Axis::AnalysisNoise => "analysis-noise",
        }
    }
}

/// One axis measurement for a target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisMeasurement {
    pub axis: Axis,
    /// Exposure before hardening, in `[0, 1]`.
    pub baseline: f64,
    /// Exposure after hardening, in `[0, 1]`.
    pub hardened: f64,
    /// Weight override; defaults to `Axis::weight` when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
}

impl AxisMeasurement {
    pub fn new(axis: Axis, baseline: f64, hardened: f64) -> Self {
        AxisMeasurement {
            axis,
            baseline: clamp01(baseline),
            hardened: clamp01(hardened),
            weight: None,
        }
    }

    pub fn effective_weight(&self) -> f64 {
        self.weight.unwrap_or_else(|| self.axis.weight())
    }

    /// Resistance contribution in `[0, 1]`.
    pub fn resistance(&self) -> f64 {
        1.0 - self.hardened
    }

    /// Improvement from baseline to hardened, in `[-1, 1]`.
    pub fn delta(&self) -> f64 {
        self.baseline - self.hardened
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Grade {
    A,
    B,
    C,
    D,
    F,
}

impl Grade {
    pub fn from_score(score: u8) -> Grade {
        match score {
            90..=100 => Grade::A,
            75..=89 => Grade::B,
            60..=74 => Grade::C,
            40..=59 => Grade::D,
            _ => Grade::F,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Grade::A => "A",
            Grade::B => "B",
            Grade::C => "C",
            Grade::D => "D",
            Grade::F => "F",
        }
    }
}

/// Full resistance report for a single target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResistanceReport {
    pub target: String,
    pub generated_unix: u64,
    pub measurements: Vec<AxisMeasurement>,
    pub rrs: u8,
    pub grade: Grade,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl ResistanceReport {
    pub fn new(
        target: impl Into<String>,
        measurements: Vec<AxisMeasurement>,
        notes: Vec<String>,
    ) -> Self {
        let rrs = compute_rrs(&measurements);
        ResistanceReport {
            target: target.into(),
            generated_unix: now_unix(),
            measurements,
            rrs,
            grade: Grade::from_score(rrs),
            notes,
        }
    }
}

/// Weighted mean of per-axis resistance, scaled to 0-100.
pub fn compute_rrs(measurements: &[AxisMeasurement]) -> u8 {
    let total_weight: f64 = measurements.iter().map(|m| m.effective_weight()).sum();
    if total_weight <= 0.0 {
        return 0;
    }
    let weighted: f64 = measurements
        .iter()
        .map(|m| m.effective_weight() * m.resistance())
        .sum();
    let score = (weighted / total_weight) * 100.0;
    score.round().clamp(0.0, 100.0) as u8
}

fn clamp01(v: f64) -> f64 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, 1.0)
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_hardening_scores_100() {
        let m = vec![
            AxisMeasurement::new(Axis::SymbolExposure, 1.0, 0.0),
            AxisMeasurement::new(Axis::StringExposure, 1.0, 0.0),
        ];
        assert_eq!(compute_rrs(&m), 100);
    }

    #[test]
    fn no_hardening_scores_zero() {
        let m = vec![
            AxisMeasurement::new(Axis::SymbolExposure, 1.0, 1.0),
            AxisMeasurement::new(Axis::StringExposure, 1.0, 1.0),
        ];
        assert_eq!(compute_rrs(&m), 0);
    }

    #[test]
    fn weights_are_applied() {
        // Agent-deception (weight 2.0) fully resisted; strings fully exposed.
        let m = vec![
            AxisMeasurement::new(Axis::AgentDeception, 1.0, 0.0),
            AxisMeasurement::new(Axis::StringExposure, 1.0, 1.0),
        ];
        // 2.0 / (2.0 + 1.5) = 0.571 -> 57
        assert_eq!(compute_rrs(&m), 57);
    }

    #[test]
    fn grades_map() {
        assert_eq!(Grade::from_score(95), Grade::A);
        assert_eq!(Grade::from_score(61), Grade::C);
        assert_eq!(Grade::from_score(10), Grade::F);
    }

    #[test]
    fn empty_is_zero() {
        assert_eq!(compute_rrs(&[]), 0);
    }
}
