//! `direa-core`: manifest, policy, adapter SDK, planner and resistance model
//! for the die-a-rea project.
//!
//! The core is deliberately small and language-agnostic. Language-specific
//! hardening lives in adapters (see [`adapter`]); this crate owns the policy
//! model, target detection, planning, and the reverse-engineering Resistance
//! Score ([`rrs`]).

pub mod adapter;
pub mod buildmeta;
pub mod error;
pub mod manifest;
pub mod native_post;
pub mod planner;
pub mod policy;
pub mod random;
pub mod rrs;

pub use adapter::{
    which, AdapterEntry, AdapterOp, AdapterRequest, AdapterResponse, Finding, PlanStep, Severity,
    Status, SubprocessAdapter,
};
pub use buildmeta::{BuildMetadata, InjectedStats, TargetMetadata};
pub use error::{DireaError, Result};
pub use manifest::{AgentDefense, Manifest, Project, Randomization, TargetSpec, Tarpit};
pub use native_post::{NativeFormat, StripReport};
pub use planner::{detect_kind, plan_manifest, BuildPlan, TargetKind};
pub use policy::{PolicyOverrides, Profile, ResolvedPolicy};
pub use random::{derive_seed, Rng};
pub use rrs::{Axis, AxisMeasurement, Grade, ResistanceReport};
