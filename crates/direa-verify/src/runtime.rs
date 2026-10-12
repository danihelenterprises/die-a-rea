//! Runtime-instrumentation axis.
//!
//! `verify` deliberately does not execute or attach to the target (ADR-0001), so
//! it cannot instrument the artifact itself. Instead, with `--with-runtime` it
//! checks whether the *analysis environment* is already instrumented (debugger,
//! Frida, V8 inspector): if so, runtime instrumentation clearly succeeds and the
//! artifact is exposed. If nothing is detected the environment is simply
//! *unknown*, so the axis is skipped rather than scored as resisted
//! (ADR-0011, K-004).

use direa_core::guard::GuardReport;
use direa_core::{Axis, AxisMeasurement};

/// Map a guard report to the runtime axis, or `None` when nothing was detected.
pub fn runtime_measurement(report: &GuardReport) -> Option<AxisMeasurement> {
    if report.instrumented {
        Some(AxisMeasurement::new(
            Axis::RuntimeInstrumentationExposure,
            1.0,
            1.0,
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instrumented_environment_is_exposed() {
        let report = GuardReport {
            instrumented: true,
            findings: vec!["TracerPid=1".into()],
        };
        let m = runtime_measurement(&report).expect("axis must appear when instrumented");
        assert_eq!(m.axis, Axis::RuntimeInstrumentationExposure);
        assert_eq!(m.hardened, 1.0);
    }

    #[test]
    fn clean_environment_skips_the_axis() {
        let report = GuardReport::default();
        assert!(runtime_measurement(&report).is_none());
    }
}
