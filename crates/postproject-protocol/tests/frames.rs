//! Incremental framing rejects truncation and excessive lengths before replay.
use postproject_protocol::{FailureKind, FrameDecoder, Limits};

fn frame(bytes: &[u8]) -> Vec<u8> {
    let mut framed = u64::try_from(bytes.len()).unwrap().to_be_bytes().to_vec();
    framed.extend_from_slice(bytes);
    framed
}

#[test]
fn documents_survive_every_header_and_payload_split_without_collecting_the_stream() {
    let bytes = [frame(br#"{"a":"\u0001"}"#), frame(br#"{"b":[]}"#)].concat();
    for size in 1..=bytes.len() {
        let mut decoder = FrameDecoder::new(Limits::default());
        let mut documents = Vec::new();
        for input in bytes.chunks(size) {
            let mut position = 0;
            while position < input.len() {
                let (count, document) = decoder.consume(&input[position..]).unwrap();
                assert!(count > 0);
                position += count;
                if let Some(document) = document {
                    documents.push(document.canonical_bytes().unwrap());
                }
            }
        }
        decoder.finish().unwrap();
        assert_eq!(
            documents,
            [br#"{"a":"\u0001"}"#.to_vec(), br#"{"b":[]}"#.to_vec()]
        );
        assert!(decoder.consume(b"x").is_err());
    }
}

#[test]
fn invalid_lengths_noncanonical_json_and_truncation_close_the_decoder() {
    for bytes in [
        frame(b"{ \"a\":\"x\"}"),
        frame(b"{}{}"),
        frame(b""),
        u64::MAX.to_be_bytes().to_vec(),
    ] {
        let mut decoder = FrameDecoder::new(Limits::default());
        assert!(decoder.consume(&bytes).is_err());
        assert!(decoder.consume(&frame(b"{}")).is_err());
    }
    let bytes = frame(br#"{"a":"long"}"#);
    for length in 1..bytes.len() {
        let mut decoder = FrameDecoder::new(Limits::default());
        decoder.consume(&bytes[..length]).unwrap();
        assert!(decoder.finish().is_err());
    }
    let mut decoder = FrameDecoder::new(Limits::new(1, 1, 1).unwrap());
    assert_eq!(
        decoder.consume(&frame(b"{}")).unwrap_err().kind(),
        FailureKind::LimitExceeded
    );
}
