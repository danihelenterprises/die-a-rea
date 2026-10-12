use serde::{Deserialize, Serialize};

/// Preset protection profiles, ordered from lightest to strongest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    Low,
    Medium,
    #[default]
    High,
    Paranoid,
}

impl Profile {
    pub fn as_str(self) -> &'static str {
        match self {
            Profile::Low => "low",
            Profile::Medium => "medium",
            Profile::High => "high",
            Profile::Paranoid => "paranoid",
        }
    }

    /// Expand a profile into a concrete set of switches.
    ///
    /// The profiles are deliberately conservative: heavier, behaviour-affecting
    /// transforms (control-flow flattening, packing) are opt-in via higher
    /// profiles so that a `low` build is always safe to ship.
    pub fn resolve(self) -> ResolvedPolicy {
        let mut p = ResolvedPolicy {
            profile: self,
            ..ResolvedPolicy::default()
        };
        match self {
            Profile::Low => {}
            Profile::Medium => {
                p.strip_debug_info = true;
                p.encrypt_strings = true;
                p.anti_debug = true;
                p.agent_canaries = true;
            }
            Profile::High => {
                p.strip_debug_info = true;
                p.encrypt_strings = true;
                p.obfuscate_control_flow = true;
                p.anti_debug = true;
                p.anti_instrumentation = true;
                p.integrity_selfcheck = true;
                p.agent_canaries = true;
                p.randomize = true;
                p.decoy_generation = true;
                p.tarpit = true;
            }
            Profile::Paranoid => {
                p.strip_debug_info = true;
                p.encrypt_strings = true;
                p.obfuscate_control_flow = true;
                p.obfuscate_data_flow = true;
                p.anti_debug = true;
                p.anti_instrumentation = true;
                p.integrity_selfcheck = true;
                p.pack = true;
                p.agent_canaries = true;
                p.agent_decoys = true;
                p.randomize = true;
                p.decoy_generation = true;
                p.tarpit = true;
            }
        }
        p
    }
}

/// Fully-resolved, concrete protection switches used by the planner and adapters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedPolicy {
    pub profile: Profile,

    // Static / artifact layer.
    pub strip_symbols: bool,
    pub strip_debug_info: bool,
    pub strip_build_metadata: bool,
    pub remove_source_maps: bool,
    pub encrypt_strings: bool,
    pub obfuscate_control_flow: bool,
    pub obfuscate_data_flow: bool,
    pub pack: bool,

    // Randomization / deception layer.
    pub randomize: bool,
    pub decoy_generation: bool,
    pub tarpit: bool,

    // Runtime layer.
    pub anti_debug: bool,
    pub anti_instrumentation: bool,
    pub integrity_selfcheck: bool,

    // Agent layer (REA-specific).
    pub agent_stop_notice: bool,
    pub agent_canaries: bool,
    pub agent_decoys: bool,
}

impl Default for ResolvedPolicy {
    fn default() -> Self {
        ResolvedPolicy {
            profile: Profile::Low,
            strip_symbols: true,
            strip_debug_info: false,
            strip_build_metadata: true,
            remove_source_maps: true,
            encrypt_strings: false,
            obfuscate_control_flow: false,
            obfuscate_data_flow: false,
            pack: false,
            randomize: false,
            decoy_generation: false,
            tarpit: false,
            anti_debug: false,
            anti_instrumentation: false,
            integrity_selfcheck: false,
            agent_stop_notice: true,
            agent_canaries: false,
            agent_decoys: false,
        }
    }
}

/// Optional per-field overrides applied on top of a profile.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyOverrides {
    pub strip_symbols: Option<bool>,
    pub strip_debug_info: Option<bool>,
    pub strip_build_metadata: Option<bool>,
    pub remove_source_maps: Option<bool>,
    pub encrypt_strings: Option<bool>,
    pub obfuscate_control_flow: Option<bool>,
    pub obfuscate_data_flow: Option<bool>,
    pub pack: Option<bool>,
    pub randomize: Option<bool>,
    pub decoy_generation: Option<bool>,
    pub tarpit: Option<bool>,
    pub anti_debug: Option<bool>,
    pub anti_instrumentation: Option<bool>,
    pub integrity_selfcheck: Option<bool>,
    pub agent_stop_notice: Option<bool>,
    pub agent_canaries: Option<bool>,
    pub agent_decoys: Option<bool>,
}

impl PolicyOverrides {
    pub fn apply(self, policy: &mut ResolvedPolicy) {
        macro_rules! set {
            ($($field:ident),* $(,)?) => {
                $(if let Some(v) = self.$field { policy.$field = v; })*
            };
        }
        set!(
            strip_symbols,
            strip_debug_info,
            strip_build_metadata,
            remove_source_maps,
            encrypt_strings,
            obfuscate_control_flow,
            obfuscate_data_flow,
            pack,
            randomize,
            decoy_generation,
            tarpit,
            anti_debug,
            anti_instrumentation,
            integrity_selfcheck,
            agent_stop_notice,
            agent_canaries,
            agent_decoys,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_are_monotonic() {
        let low = Profile::Low.resolve();
        let high = Profile::High.resolve();
        let paranoid = Profile::Paranoid.resolve();
        assert!(!low.obfuscate_control_flow);
        assert!(high.obfuscate_control_flow);
        assert!(paranoid.pack);
        assert!(high.anti_instrumentation);
        assert!(!low.anti_instrumentation);
        assert!(high.randomize);
        assert!(high.decoy_generation);
        assert!(high.tarpit);
        assert!(!low.randomize);
        assert!(paranoid.agent_decoys);
    }

    #[test]
    fn overrides_win() {
        let mut policy = Profile::Low.resolve();
        PolicyOverrides {
            obfuscate_control_flow: Some(true),
            agent_decoys: Some(true),
            ..Default::default()
        }
        .apply(&mut policy);
        assert!(policy.obfuscate_control_flow);
        assert!(policy.agent_decoys);
    }
}
