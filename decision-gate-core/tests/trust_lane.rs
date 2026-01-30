// decision-gate-core/tests/trust_lane.rs
// ============================================================================
// Module: Trust Lane and Requirement Tests
// Description: Comprehensive tests for trust lane ranking, satisfaction, and
//              the stricter() lattice composition across config, condition,
//              and gate levels.
// Purpose: Ensure trust policies fail closed and compose correctly.
// Dependencies: decision-gate-core
// ============================================================================
//! ## Overview
//! Validates trust lane rank ordering, satisfaction semantics, and the lattice
//! properties of `TrustRequirement::stricter()`.
//!
//! Security posture: Trust lanes enforce fail-closed policies; stricter always wins.
//! Threat model: TM-TRUST-001 - Trust lane bypass via incorrect composition.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test-only panic-based assertions are permitted."
)]

use decision_gate_core::TrustLane;
use decision_gate_core::TrustRequirement;

// ============================================================================
// SECTION: Trust Lane Rank Ordering
// ============================================================================

#[test]
fn trust_lane_verified_rank_is_one() {
    assert!(TrustLane::Verified.satisfies(TrustRequirement {
        min_lane: TrustLane::Verified
    }));
    // Verified has rank 1 (higher than Asserted's 0)
}

#[test]
fn trust_lane_asserted_rank_is_zero() {
    // Asserted (rank 0) does not satisfy Verified requirement (rank 1)
    assert!(!TrustLane::Asserted.satisfies(TrustRequirement {
        min_lane: TrustLane::Verified
    }));
}

#[test]
fn trust_lane_verified_rank_greater_than_asserted() {
    // Verified always satisfies what Asserted satisfies, plus more
    let asserted_req = TrustRequirement {
        min_lane: TrustLane::Asserted,
    };
    let verified_req = TrustRequirement {
        min_lane: TrustLane::Verified,
    };

    // Verified satisfies both requirements
    assert!(TrustLane::Verified.satisfies(asserted_req));
    assert!(TrustLane::Verified.satisfies(verified_req));

    // Asserted only satisfies asserted requirement
    assert!(TrustLane::Asserted.satisfies(asserted_req));
    assert!(!TrustLane::Asserted.satisfies(verified_req));
}

// ============================================================================
// SECTION: Trust Lane Satisfies Matrix
// ============================================================================

#[test]
fn trust_lane_verified_satisfies_verified_requirement() {
    let requirement = TrustRequirement {
        min_lane: TrustLane::Verified,
    };
    assert!(TrustLane::Verified.satisfies(requirement));
}

#[test]
fn trust_lane_verified_satisfies_asserted_requirement() {
    let requirement = TrustRequirement {
        min_lane: TrustLane::Asserted,
    };
    assert!(TrustLane::Verified.satisfies(requirement));
}

#[test]
fn trust_lane_asserted_satisfies_asserted_requirement() {
    let requirement = TrustRequirement {
        min_lane: TrustLane::Asserted,
    };
    assert!(TrustLane::Asserted.satisfies(requirement));
}

#[test]
fn trust_lane_asserted_does_not_satisfy_verified_requirement() {
    let requirement = TrustRequirement {
        min_lane: TrustLane::Verified,
    };
    assert!(!TrustLane::Asserted.satisfies(requirement));
}

// ============================================================================
// SECTION: Trust Lane Defaults
// ============================================================================

#[test]
fn trust_lane_default_is_verified() {
    assert_eq!(TrustLane::default(), TrustLane::Verified);
}

#[test]
fn trust_requirement_default_is_verified() {
    let default_req = TrustRequirement::default();
    assert_eq!(default_req.min_lane, TrustLane::Verified);
}

// ============================================================================
// SECTION: Trust Requirement Stricter - Lattice Properties
// ============================================================================

#[test]
fn trust_requirement_stricter_is_idempotent_verified() {
    let verified = TrustRequirement {
        min_lane: TrustLane::Verified,
    };
    assert_eq!(verified.stricter(verified), verified);
}

#[test]
fn trust_requirement_stricter_is_idempotent_asserted() {
    let asserted = TrustRequirement {
        min_lane: TrustLane::Asserted,
    };
    assert_eq!(asserted.stricter(asserted), asserted);
}

#[test]
fn trust_requirement_stricter_is_commutative() {
    let verified = TrustRequirement {
        min_lane: TrustLane::Verified,
    };
    let asserted = TrustRequirement {
        min_lane: TrustLane::Asserted,
    };

    // stricter(a, b) == stricter(b, a)
    assert_eq!(verified.stricter(asserted), asserted.stricter(verified));
}

#[test]
fn trust_requirement_stricter_is_associative() {
    let verified = TrustRequirement {
        min_lane: TrustLane::Verified,
    };
    let asserted = TrustRequirement {
        min_lane: TrustLane::Asserted,
    };

    // stricter(a, stricter(b, c)) == stricter(stricter(a, b), c)
    // With only two lanes, test all combinations
    let left = verified.stricter(asserted.stricter(verified));
    let right = verified.stricter(asserted).stricter(verified);
    assert_eq!(left, right);

    let left2 = asserted.stricter(verified.stricter(asserted));
    let right2 = asserted.stricter(verified).stricter(asserted);
    assert_eq!(left2, right2);
}

#[test]
fn trust_requirement_stricter_verified_asserted_returns_verified() {
    let verified = TrustRequirement {
        min_lane: TrustLane::Verified,
    };
    let asserted = TrustRequirement {
        min_lane: TrustLane::Asserted,
    };

    let result = verified.stricter(asserted);
    assert_eq!(result.min_lane, TrustLane::Verified);
}

#[test]
fn trust_requirement_stricter_asserted_verified_returns_verified() {
    let verified = TrustRequirement {
        min_lane: TrustLane::Verified,
    };
    let asserted = TrustRequirement {
        min_lane: TrustLane::Asserted,
    };

    let result = asserted.stricter(verified);
    assert_eq!(result.min_lane, TrustLane::Verified);
}

// ============================================================================
// SECTION: Three-Level Composition (Config -> Condition -> Gate)
// ============================================================================

/// Helper to compose three trust requirements in order: config -> condition -> gate
fn compose_three_levels(
    config: TrustRequirement,
    condition: Option<TrustRequirement>,
    gate: Option<TrustRequirement>,
) -> TrustRequirement {
    let with_condition = condition.map_or(config, |p| config.stricter(p));
    gate.map_or(with_condition, |g| with_condition.stricter(g))
}

#[test]
fn three_level_config_asserted_condition_verified_gate_none_yields_verified() {
    let config = TrustRequirement {
        min_lane: TrustLane::Asserted,
    };
    let condition = Some(TrustRequirement {
        min_lane: TrustLane::Verified,
    });
    let gate = None;

    let result = compose_three_levels(config, condition, gate);
    assert_eq!(result.min_lane, TrustLane::Verified);
}

#[test]
fn three_level_config_verified_condition_asserted_gate_verified_yields_verified() {
    let config = TrustRequirement {
        min_lane: TrustLane::Verified,
    };
    let condition = Some(TrustRequirement {
        min_lane: TrustLane::Asserted,
    });
    let gate = Some(TrustRequirement {
        min_lane: TrustLane::Verified,
    });

    let result = compose_three_levels(config, condition, gate);
    assert_eq!(result.min_lane, TrustLane::Verified);
}

#[test]
fn three_level_config_asserted_condition_none_gate_asserted_yields_asserted() {
    let config = TrustRequirement {
        min_lane: TrustLane::Asserted,
    };
    let condition = None;
    let gate = Some(TrustRequirement {
        min_lane: TrustLane::Asserted,
    });

    let result = compose_three_levels(config, condition, gate);
    assert_eq!(result.min_lane, TrustLane::Asserted);
}

#[test]
fn three_level_all_verified_yields_verified() {
    let config = TrustRequirement {
        min_lane: TrustLane::Verified,
    };
    let condition = Some(TrustRequirement {
        min_lane: TrustLane::Verified,
    });
    let gate = Some(TrustRequirement {
        min_lane: TrustLane::Verified,
    });

    let result = compose_three_levels(config, condition, gate);
    assert_eq!(result.min_lane, TrustLane::Verified);
}

#[test]
fn three_level_all_asserted_yields_asserted() {
    let config = TrustRequirement {
        min_lane: TrustLane::Asserted,
    };
    let condition = Some(TrustRequirement {
        min_lane: TrustLane::Asserted,
    });
    let gate = Some(TrustRequirement {
        min_lane: TrustLane::Asserted,
    });

    let result = compose_three_levels(config, condition, gate);
    assert_eq!(result.min_lane, TrustLane::Asserted);
}

#[test]
fn three_level_default_stacks_correctly_with_asserted_override() {
    // Default is Verified; if condition says Asserted, config's Verified wins
    let config = TrustRequirement::default(); // Verified
    let condition = Some(TrustRequirement {
        min_lane: TrustLane::Asserted,
    });
    let gate = None;

    let result = compose_three_levels(config, condition, gate);
    assert_eq!(result.min_lane, TrustLane::Verified);
}

#[test]
fn three_level_gate_can_tighten_relaxed_condition() {
    // Even if config and condition allow Asserted, gate can require Verified
    let config = TrustRequirement {
        min_lane: TrustLane::Asserted,
    };
    let condition = Some(TrustRequirement {
        min_lane: TrustLane::Asserted,
    });
    let gate = Some(TrustRequirement {
        min_lane: TrustLane::Verified,
    });

    let result = compose_three_levels(config, condition, gate);
    assert_eq!(result.min_lane, TrustLane::Verified);
}

#[test]
fn three_level_condition_cannot_relax_config() {
    // Config says Verified; condition saying Asserted cannot relax it
    let config = TrustRequirement {
        min_lane: TrustLane::Verified,
    };
    let condition = Some(TrustRequirement {
        min_lane: TrustLane::Asserted,
    });
    let gate = None;

    let result = compose_three_levels(config, condition, gate);
    assert_eq!(result.min_lane, TrustLane::Verified);
}

#[test]
fn three_level_gate_cannot_relax_condition() {
    // Condition says Verified; gate saying Asserted cannot relax it
    let config = TrustRequirement {
        min_lane: TrustLane::Asserted,
    };
    let condition = Some(TrustRequirement {
        min_lane: TrustLane::Verified,
    });
    let gate = Some(TrustRequirement {
        min_lane: TrustLane::Asserted,
    });

    let result = compose_three_levels(config, condition, gate);
    assert_eq!(result.min_lane, TrustLane::Verified);
}

// ============================================================================
// SECTION: Serialization Round-Trip
// ============================================================================

#[test]
fn trust_lane_serde_roundtrip_verified() {
    let lane = TrustLane::Verified;
    let serialized = serde_json::to_string(&lane).unwrap();
    assert_eq!(serialized, "\"verified\"");
    let deserialized: TrustLane = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, lane);
}

#[test]
fn trust_lane_serde_roundtrip_asserted() {
    let lane = TrustLane::Asserted;
    let serialized = serde_json::to_string(&lane).unwrap();
    assert_eq!(serialized, "\"asserted\"");
    let deserialized: TrustLane = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, lane);
}

#[test]
fn trust_requirement_serde_roundtrip() {
    let req = TrustRequirement {
        min_lane: TrustLane::Verified,
    };
    let serialized = serde_json::to_string(&req).unwrap();
    let deserialized: TrustRequirement = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, req);
}

#[test]
fn trust_lane_invalid_value_rejected() {
    let result: Result<TrustLane, _> = serde_json::from_str("\"unknown\"");
    assert!(result.is_err());
}
