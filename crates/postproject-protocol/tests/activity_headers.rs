//! Provenance scalars retain exact attribution and signed time boundaries.

use postproject_core::{
    Activity, ActivityId, ActivityKind, ActivityOutput, AgentIdentity, ExternalIdentifier,
    IdentifierScheme, RepresentationId, Timestamp, ToolIdentity,
};
use postproject_protocol::{ActivityHeader, Document, Limits};

fn header() -> ActivityHeader {
    ActivityHeader::new(
        ActivityId::new(),
        ActivityKind::new("unknown:ExAct").unwrap(),
        0,
        1,
    )
    .unwrap()
    .with_timing(
        Some(Timestamp::from_unix_micros(1)),
        Some(Timestamp::from_unix_micros(2)),
    )
    .unwrap()
    .with_tool(
        ToolIdentity::new(
            " 工具 ",
            Some("Exact.1".into()),
            Some("https://example.invalid/%2f".into()),
        )
        .unwrap(),
    )
    .with_agent(
        AgentIdentity::new(
            Some(" agent名 ".into()),
            Some(
                ExternalIdentifier::new(
                    IdentifierScheme::new("unknown:Agent").unwrap(),
                    "worker%2f名",
                    Some("Case".into()),
                )
                .unwrap(),
            ),
        )
        .unwrap(),
    )
}

#[test]
fn scalar_headers_borrow_native_edges_and_preserve_all_optional_attribution() {
    for (started, finished) in [
        (None, None),
        (Some(i64::MIN), Some(i64::MAX)),
        (Some(9_007_199_254_740_993), None),
        (None, Some(-1)),
    ] {
        let mut activity = Activity::new(
            ActivityId::new(),
            ActivityKind::new("unknown:Generator").unwrap(),
            Vec::new(),
            vec![ActivityOutput::new(RepresentationId::new(), None)],
        )
        .unwrap()
        .with_timing(
            started.map(Timestamp::from_unix_micros),
            finished.map(Timestamp::from_unix_micros),
        )
        .unwrap();
        for attributed in [false, true] {
            if attributed {
                let facts = header();
                activity = activity
                    .with_tool(facts.tool().unwrap().clone())
                    .with_agent(facts.agent().unwrap().clone());
            }
            let header = ActivityHeader::from_activity(&activity).unwrap();
            assert_eq!(header.id(), activity.id());
            assert_eq!(header.kind(), activity.kind());
            assert_eq!(header.started_at(), activity.started_at());
            assert_eq!(header.finished_at(), activity.finished_at());
            assert_eq!(header.tool(), activity.tool());
            assert_eq!(header.agent(), activity.agent());
            assert_eq!(header.input_count(), 0);
            assert_eq!(header.output_count(), 1);
            let bytes = header.document().canonical_bytes().unwrap();
            assert_eq!(
                ActivityHeader::from_document(&Document::parse(&bytes, Limits::default()).unwrap())
                    .unwrap(),
                header
            );
        }
    }
    let maximum = ActivityHeader::new(
        ActivityId::new(),
        ActivityKind::new("unknown:Maximum").unwrap(),
        100_000,
        100_000,
    )
    .unwrap();
    assert_eq!(
        ActivityHeader::from_document(&maximum.document()).unwrap(),
        maximum
    );
}

#[test]
fn malformed_headers_cannot_bypass_native_counts_timing_or_attribution_rules() {
    let document: serde_json::Value =
        serde_json::from_slice(&header().document().canonical_bytes().unwrap()).unwrap();
    let contradictions: [(&[&str], serde_json::Value); 12] = [
        (&["output_count"], "0".into()),
        (&["input_count"], "100001".into()),
        (&["output_count"], "100001".into()),
        (&["input_count"], serde_json::json!(0)),
        (&["finished_at_micros"], "0".into()),
        (&["started_at_micros"], "9223372036854775808".into()),
        (&["activity_kind"], "".into()),
        (&["tool", "name"], "bad\0".into()),
        (&["tool", "version"], "".into()),
        (&["agent", "name"], "".into()),
        (&["agent", "identifier", "value"], "".into()),
        (&["unknown"], serde_json::Value::Null),
    ];
    for (path, value) in contradictions {
        let mut changed = document.clone();
        let mut field = &mut changed;
        for key in path {
            field = &mut field[*key];
        }
        *field = value;
        assert!(
            Document::parse(&serde_json::to_vec(&changed).unwrap(), Limits::default())
                .and_then(|document| ActivityHeader::from_document(&document))
                .is_err()
        );
    }
}
