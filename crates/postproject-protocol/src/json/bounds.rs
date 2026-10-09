//! Match parser node/depth accounting for already constructed documents.

use serde_json::Value;

use super::Limits;
use crate::{Result, fields::limit};

pub(super) fn validate(value: &Value, limits: Limits) -> Result<()> {
    visit(value, 0, &mut 0, limits)
}

fn visit(value: &Value, depth: usize, nodes: &mut usize, limits: Limits) -> Result<()> {
    if *nodes == limits.nodes {
        return Err(limit());
    }
    *nodes += 1;
    match value {
        Value::Array(values) => {
            if depth >= limits.depth {
                return Err(limit());
            }
            for value in values {
                visit(value, depth + 1, nodes, limits)?;
            }
        }
        Value::Object(values) => {
            if depth >= limits.depth {
                return Err(limit());
            }
            for value in values.values() {
                visit(value, depth + 1, nodes, limits)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;
    use serde_json::json;

    #[test]
    fn constructed_and_parsed_documents_agree_at_exact_node_depth_and_byte_boundaries() {
        for value in [
            json!({"a":true}),
            json!({"a":[true,null]}),
            json!({"a":{"b":"名"}}),
            json!({"a":[{"b":false}]}),
        ] {
            let document = Document { value };
            let bytes = document.canonical_bytes().unwrap();
            for nodes in 1..=8 {
                for depth in 1..=4 {
                    for byte_limit in [bytes.len() - 1, bytes.len(), bytes.len() + 1] {
                        let limits = Limits::new(byte_limit, depth, nodes).unwrap();
                        let built = document.bounded_canonical_bytes(limits);
                        let parsed = Document::parse(&bytes, limits);
                        assert_eq!(
                            built.as_ref().err().map(crate::ProtocolError::kind),
                            parsed.as_ref().err().map(crate::ProtocolError::kind)
                        );
                        if let Ok(built) = built {
                            assert_eq!(built, bytes);
                        }
                    }
                }
            }
        }
    }
}
