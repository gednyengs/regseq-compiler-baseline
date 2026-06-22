//! Fingerprint + simple grouping per strategy section 8.
//!
//! F1 keeps clustering intentionally simple: hash the fingerprint
//! tuple via `DefaultHasher`, bucket by hash, sort members within
//! each cluster by snippet length (shortest first) so the
//! representative repro is the shortest member. Clusters are
//! sorted by member count descending.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use regseq_driver::bridge::BridgeError;

use crate::matrix::Expectation;
use crate::oracle::OracleOutcome;

/// Per-row result fed into [`group`].
#[derive(Debug)]
pub struct RowResult {
    /// Owning row ID.
    pub row_id: String,
    /// Row's expectation class.
    pub expectation: Expectation,
    /// Explicit cluster parent from the sidecar.
    pub cluster_key: String,
    /// Snippet path for display.
    pub snippet_path: PathBuf,
    /// Snippet length in bytes (for shortest-member selection).
    pub byte_len: usize,
    /// Snippet hash prefix per [`crate::snippet::Snippet::hash_prefix`].
    pub snippet_hash: String,
    /// Empirical outcome variant tag for the fingerprint.
    pub actual_outcome: ActualOutcomeTag,
    /// Variant or phase string (BridgeError variant name / panic
    /// top-line / etc).
    pub variant_or_phase: String,
    /// Pass / fail decision per strategy section 9 + Change G of
    /// the F1 task spec.
    pub pass: bool,
    /// One-line human-readable detail surfaced in the report.
    pub detail: String,
}

/// Empirical outcome classifier per strategy section 8.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ActualOutcomeTag {
    /// Bridge succeeded.
    Ok,
    /// Bridge surfaced a typed `BridgeError`.
    Rejected,
    /// Validator surfaced an `InvariantViolation`.
    ValidatorFailed,
    /// Rustc subprocess clean-rejected the snippet.
    RustcCleanReject,
    /// Rustc subprocess emitted an ICE.
    RustcICE,
    /// In-process bridge ran but never reached `after_analysis`.
    RustcNoAfterAnalysis,
    /// Bridge panicked.
    Panic,
}

impl ActualOutcomeTag {
    fn as_static_str(&self) -> &'static str {
        match self {
            ActualOutcomeTag::Ok => "Ok",
            ActualOutcomeTag::Rejected => "Rejected",
            ActualOutcomeTag::ValidatorFailed => "ValidatorFailed",
            ActualOutcomeTag::RustcCleanReject => "RustcCleanReject",
            ActualOutcomeTag::RustcICE => "RustcICE",
            ActualOutcomeTag::RustcNoAfterAnalysis => "RustcNoAfterAnalysis",
            ActualOutcomeTag::Panic => "Panic",
        }
    }
}

/// One fingerprinted cluster of rows that share an outcome shape.
#[derive(Debug)]
pub struct Cluster {
    /// Hash of the fingerprint tuple.
    pub fingerprint_hash: u64,
    /// Snapshot of one member's fingerprint for the report header.
    pub fingerprint: Fingerprint,
    /// Members sorted by snippet length (shortest first).
    pub members: Vec<ClusterMember>,
}

/// Plain-data fingerprint snapshot used to render the cluster
/// header in the report.
#[derive(Debug, Clone)]
pub struct Fingerprint {
    /// Explicit cluster parent from the sidecar.
    pub cluster_key: String,
    /// Expectation class.
    pub expectation_class: Expectation,
    /// Empirical outcome tag.
    pub actual_outcome: ActualOutcomeTag,
    /// Variant or phase string.
    pub variant_or_phase: String,
}

/// One member of a [`Cluster`].
#[derive(Debug)]
pub struct ClusterMember {
    /// Owning row ID.
    pub row_id: String,
    /// Snippet path.
    pub snippet_path: PathBuf,
    /// Snippet length in bytes.
    pub byte_len: usize,
    /// One-line detail string from the [`RowResult`].
    pub detail: String,
    /// Pass / fail flag for the row.
    pub pass: bool,
}

/// Bucket `results` into clusters by fingerprint hash. Sort
/// clusters by member count descending.
pub fn group(results: &[RowResult]) -> Vec<Cluster> {
    let mut buckets: HashMap<u64, Cluster> = HashMap::new();
    for r in results {
        let fingerprint = Fingerprint {
            cluster_key: r.cluster_key.clone(),
            expectation_class: r.expectation,
            actual_outcome: r.actual_outcome.clone(),
            variant_or_phase: r.variant_or_phase.clone(),
        };
        let mut hasher = DefaultHasher::new();
        fingerprint.cluster_key.hash(&mut hasher);
        expectation_discriminant(&fingerprint.expectation_class).hash(&mut hasher);
        fingerprint.actual_outcome.as_static_str().hash(&mut hasher);
        fingerprint.variant_or_phase.hash(&mut hasher);
        r.snippet_hash.hash(&mut hasher);
        let h = hasher.finish();
        let entry = buckets.entry(h).or_insert(Cluster {
            fingerprint_hash: h,
            fingerprint: fingerprint.clone(),
            members: Vec::new(),
        });
        entry.members.push(ClusterMember {
            row_id: r.row_id.clone(),
            snippet_path: r.snippet_path.clone(),
            byte_len: r.byte_len,
            detail: r.detail.clone(),
            pass: r.pass,
        });
    }
    let mut clusters: Vec<Cluster> = buckets.into_values().collect();
    for cluster in clusters.iter_mut() {
        cluster.members.sort_by_key(|m| m.byte_len);
    }
    clusters.sort_by(|a, b| {
        b.members
            .len()
            .cmp(&a.members.len())
            .then(a.fingerprint.cluster_key.cmp(&b.fingerprint.cluster_key))
    });
    clusters
}

fn expectation_discriminant(e: &Expectation) -> u8 {
    match e {
        Expectation::Positive => 0,
        Expectation::Rejected => 1,
        Expectation::ValidatorFailing => 2,
    }
}

/// Return the variant name string for a [`BridgeError`] value;
/// used as the `variant_or_phase` field of the fingerprint when
/// the row's outcome is `Rejected`.
pub fn bridge_error_variant(err: &BridgeError) -> &'static str {
    match err {
        BridgeError::DslRejection { .. } => "DslRejection",
        BridgeError::NonRigidTy { .. } => "NonRigidTy",
        BridgeError::Foreign { .. } => "Foreign",
        BridgeError::Coroutine { .. } => "Coroutine",
        BridgeError::Pat { .. } => "Pat",
        BridgeError::UnsafeBinder { .. } => "UnsafeBinder",
        BridgeError::DynAutoOnly { .. } => "DynAutoOnly",
        BridgeError::DynStar { .. } => "DynStar",
        BridgeError::CustomAutoTrait { .. } => "CustomAutoTrait",
        BridgeError::SubstantiveHrtb { .. } => "SubstantiveHrtb",
        BridgeError::Layout { .. } => "Layout",
        BridgeError::UnsupportedEnumLayout { .. } => "UnsupportedEnumLayout",
        BridgeError::UnsupportedTy { .. } => "UnsupportedTy",
        BridgeError::UnsupportedAbi { .. } => "UnsupportedAbi",
        BridgeError::UnsupportedNeverInput { .. } => "UnsupportedNeverInput",
        BridgeError::UnsupportedCAbiType { .. } => "UnsupportedCAbiType",
        BridgeError::UnsupportedConst { .. } => "UnsupportedConst",
        BridgeError::GenericOpaqueHandle(_) => "GenericOpaqueHandle",
        BridgeError::Phase1(_) => "Phase1",
        BridgeError::Phase3(_) => "Phase3",
        BridgeError::Phase4(_) => "Phase4",
        BridgeError::Phase5(_) => "Phase5",
        BridgeError::DiagLogFormat { .. } => "DiagLogFormat",
        BridgeError::DiagLogValidation { .. } => "DiagLogValidation",
        BridgeError::UnsupportedQuestionMark { .. } => "UnsupportedQuestionMark",
        BridgeError::DeviceLifecycleUnsupportedBus { .. } => "DeviceLifecycleUnsupportedBus",
        BridgeError::DeviceLifecycleValidation(_) => "DeviceLifecycleValidation",
        BridgeError::InterruptValidation(_) => "InterruptValidation",
        BridgeError::ClockValidation(_) => "ClockValidation",
        BridgeError::PinctrlValidation(_) => "PinctrlValidation",
        BridgeError::TimedCpuDelayValidation(_) => "TimedCpuDelayValidation",
        BridgeError::AtomicValidation(_) => "AtomicValidation",
        BridgeError::SerialValidation(_) => "SerialValidation",
        BridgeError::SynchronizationValidation(_) => "SynchronizationValidation",
    }
}

/// Variant tag for the `OracleOutcome` enum; used to render
/// per-row summary text.
pub fn oracle_outcome_tag(outcome: &OracleOutcome) -> ActualOutcomeTag {
    match outcome {
        OracleOutcome::Ok(_) => ActualOutcomeTag::Ok,
        OracleOutcome::ValidatorFailed { .. } => ActualOutcomeTag::ValidatorFailed,
        OracleOutcome::Rejected(_) => ActualOutcomeTag::Rejected,
        OracleOutcome::RustcCleanReject { .. } => ActualOutcomeTag::RustcCleanReject,
        OracleOutcome::RustcICE { .. } => ActualOutcomeTag::RustcICE,
        OracleOutcome::RustcNoAfterAnalysis => ActualOutcomeTag::RustcNoAfterAnalysis,
        OracleOutcome::Panic { .. } => ActualOutcomeTag::Panic,
    }
}
