use std::{
    io,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
};

use academic_rpc::{
    FrameClass, FrameSection, RpcError, decode_envelope_frame,
    frame::{decode_exact_frame, encode_frame, read_frame, write_frame},
    generated::{MutableRequest, SyntheticIngestCommand, mutable_request},
    limits::MAX_HANDSHAKE_FRAME_BYTES,
};
use prost::Message;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[derive(Debug)]
struct OneByteReader {
    bytes: Vec<u8>,
    cursor: usize,
}

impl OneByteReader {
    fn new(bytes: Vec<u8>) -> Self {
        Self { bytes, cursor: 0 }
    }
}

impl AsyncRead for OneByteReader {
    fn poll_read(
        self: Pin<&mut Self>,
        _context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if buffer.remaining() == 0 || this.cursor == this.bytes.len() {
            return Poll::Ready(Ok(()));
        }
        buffer.put_slice(&this.bytes[this.cursor..=this.cursor]);
        this.cursor += 1;
        Poll::Ready(Ok(()))
    }
}

#[derive(Debug, Default)]
struct OneByteWriter {
    bytes: Vec<u8>,
}

impl AsyncWrite for OneByteWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let Some(byte) = buffer.first() else {
            return Poll::Ready(Ok(0));
        };
        this.bytes.push(*byte);
        Poll::Ready(Ok(1))
    }

    fn poll_flush(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[derive(Debug)]
struct OversizeProbeReader {
    prefix: [u8; 4],
    delivered: bool,
    body_polled: Arc<AtomicBool>,
}

impl AsyncRead for OversizeProbeReader {
    fn poll_read(
        self: Pin<&mut Self>,
        _context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.delivered {
            this.body_polled.store(true, Ordering::SeqCst);
            return Poll::Ready(Err(io::Error::other("body must not be read")));
        }
        buffer.put_slice(&this.prefix);
        this.delivered = true;
        Poll::Ready(Ok(()))
    }
}

fn encode_varint(mut value: usize) -> Vec<u8> {
    let mut encoded = Vec::new();
    loop {
        let mut octet = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            octet |= 0x80;
        }
        encoded.push(octet);
        if value == 0 {
            return encoded;
        }
    }
}

fn valid_request() -> MutableRequest {
    MutableRequest {
        request_id: vec![1; 16],
        client_instance_id: vec![2; 16],
        idempotency_key: vec![3; 32],
        request_digest: vec![4; 32],
        expected_profile_revision: None,
        capability_id: "learning-platform.local.synthetic-ingest.v1".to_owned(),
        command: Some(mutable_request::Command::SyntheticIngest(
            SyntheticIngestCommand {
                synthetic_fixture_id: "signed-batch-v2".to_owned(),
            },
        )),
    }
}

#[test]
fn frame_prefix_is_big_endian() -> Result<(), Box<dyn std::error::Error>> {
    let payload = vec![0x5a; 0x0102];
    let frame = encode_frame(&payload, FrameClass::Handshake)?;
    assert_eq!(&frame[..4], &[0x00, 0x00, 0x01, 0x02]);
    Ok(())
}

#[test]
fn zero_length_frame_rejected() {
    assert_eq!(
        decode_exact_frame(&[0, 0, 0, 0], FrameClass::Handshake),
        Err(RpcError::ZeroLengthFrame)
    );
}

#[tokio::test]
async fn oversize_frame_rejected_before_allocation() {
    let declared = MAX_HANDSHAKE_FRAME_BYTES + 1;
    let prefix = u32::try_from(declared).map_or([0xff; 4], u32::to_be_bytes);
    let body_polled = Arc::new(AtomicBool::new(false));
    let mut reader = OversizeProbeReader {
        prefix,
        delivered: false,
        body_polled: Arc::clone(&body_polled),
    };
    assert_eq!(
        read_frame(&mut reader, FrameClass::Handshake).await,
        Err(RpcError::FrameTooLarge {
            declared,
            maximum: MAX_HANDSHAKE_FRAME_BYTES,
        })
    );
    assert!(!body_polled.load(Ordering::SeqCst));
}

#[test]
fn truncated_frame_rejected() {
    assert_eq!(
        decode_exact_frame(&[0, 0, 0], FrameClass::Command),
        Err(RpcError::TruncatedFrame {
            section: FrameSection::Prefix,
            expected: 4,
            received: 3,
        })
    );
    assert_eq!(
        decode_exact_frame(&[0, 0, 0, 3, 0xaa, 0xbb], FrameClass::Command),
        Err(RpcError::TruncatedFrame {
            section: FrameSection::Payload,
            expected: 3,
            received: 2,
        })
    );
}

#[tokio::test]
async fn one_byte_fragmentation_round_trips_reads_and_writes()
-> Result<(), Box<dyn std::error::Error>> {
    let payload = (0_u8..=127).collect::<Vec<_>>();
    let expected_frame = encode_frame(&payload, FrameClass::Handshake)?;
    let mut reader = OneByteReader::new(expected_frame.clone());
    assert_eq!(
        read_frame(&mut reader, FrameClass::Handshake).await?,
        payload
    );

    let mut writer = OneByteWriter::default();
    write_frame(&mut writer, &payload, FrameClass::Handshake).await?;
    assert_eq!(writer.bytes, expected_frame);
    Ok(())
}

#[test]
fn every_eof_boundary_and_trailing_byte_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
    let frame = encode_frame(&[0x08, 0x01], FrameClass::Command)?;
    for cut in 0..frame.len() {
        let Err(error) = decode_exact_frame(&frame[..cut], FrameClass::Command) else {
            return Err(format!("EOF at byte {cut} was accepted").into());
        };
        assert!(matches!(error, RpcError::TruncatedFrame { .. }));
    }
    assert_eq!(
        decode_exact_frame(&frame, FrameClass::Command)?,
        &[0x08, 0x01]
    );

    let mut trailing = frame;
    trailing.push(0);
    assert_eq!(
        decode_exact_frame(&trailing, FrameClass::Command),
        Err(RpcError::TrailingFrameData { trailing: 1 })
    );
    Ok(())
}

#[test]
fn malformed_protobuf_and_unknown_write_command_are_typed() -> Result<(), Box<dyn std::error::Error>>
{
    let malformed = encode_frame(&[0x80], FrameClass::Command)?;
    assert!(matches!(
        decode_envelope_frame(&malformed, FrameClass::Command),
        Err(RpcError::MalformedData { .. })
    ));

    let mut request_payload = valid_request().encode_to_vec();
    request_payload.extend_from_slice(&[0x6a, 0x00]); // reserved command tag 13
    let mut envelope_payload = vec![0x1a]; // LocalCoreEnvelope.mutable_request
    envelope_payload.extend_from_slice(&encode_varint(request_payload.len()));
    envelope_payload.extend_from_slice(&request_payload);
    let frame = encode_frame(&envelope_payload, FrameClass::Command)?;
    assert_eq!(
        decode_envelope_frame(&frame, FrameClass::Command),
        Err(RpcError::UnknownWriteCommand { tag: 13 })
    );
    Ok(())
}

#[test]
fn protobuf_group_depth_is_bounded() -> Result<(), Box<dyn std::error::Error>> {
    let mut payload = vec![0x0b; 17]; // nested field-1 start groups
    payload.extend_from_slice(&[0x0c; 17]); // matching end groups
    let frame = encode_frame(&payload, FrameClass::Handshake)?;
    assert_eq!(
        decode_envelope_frame(&frame, FrameClass::Handshake),
        Err(RpcError::ProtobufNestingLimitExceeded { maximum: 16 })
    );
    Ok(())
}

#[test]
fn deterministic_malformed_payload_corpus_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = 0x9e37_79b9_u32;
    for length in 1..=256 {
        let mut payload = Vec::with_capacity(length);
        for _ in 0..length {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            payload.push(state.to_be_bytes()[0]);
        }
        payload[0] = 0; // field number zero is always malformed, irrespective of the tail
        let frame = encode_frame(&payload, FrameClass::Command)?;
        assert!(matches!(
            decode_envelope_frame(&frame, FrameClass::Command),
            Err(RpcError::MalformedData { .. })
        ));
    }
    Ok(())
}
