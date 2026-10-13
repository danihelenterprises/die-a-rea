//! `direa-verify`: the measurable-resistance half of die-a-rea.
//!
//! It probes an artifact before and after hardening and emits a
//! reverse-engineering Resistance Score (RRS). The optional REA adversary runs
//! the actual `rea-agents` CLI against the hardened target and records whether
//! die-a-rea canaries leak into its Evidence.

pub mod harness;
pub mod probes;
pub mod report;
pub mod runtime;

pub use harness::{measure, run, ReaObservation, VerifyOptions, VerifyRun};
pub use probes::{
    default_probes, DebugInfoProbe, ExternalControlFlowProbe, Probe, ProbeKind, ProbeReading,
    SourceMapProbe, StringProbe, SymbolProbe,
};
