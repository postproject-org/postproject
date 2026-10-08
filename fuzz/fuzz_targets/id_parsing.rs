#![no_main]

use std::str::FromStr;

use libfuzzer_sys::fuzz_target;
use postproject_core::{
    ActivityId, AssetId, DecisionBase, JobClaimId, JobId, LocatorId, MediaRootId, ProductionId,
    RepresentationId, ResourceId, RevisionId, TransactionId,
};

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data);
    let _ = ProductionId::from_str(&input);
    let _ = AssetId::from_str(&input);
    let _ = RepresentationId::from_str(&input);
    let _ = ResourceId::from_str(&input);
    let _ = LocatorId::from_str(&input);
    let _ = MediaRootId::from_str(&input);
    let _ = TransactionId::from_str(&input);
    let _ = ActivityId::from_str(&input);
    let _ = JobId::from_str(&input);
    let _ = JobClaimId::from_str(&input);
    let _ = RevisionId::from_str(&input);
    if let Ok(base) = DecisionBase::from_str(&input) {
        assert_eq!(base.to_string(), input);
        assert_eq!(DecisionBase::from_str(&base.to_string()).unwrap(), base);
    }
});
