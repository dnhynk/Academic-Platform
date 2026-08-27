//! Bounded in-memory framing with no OS transport behavior.

use prost::Message;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{
    error::{FrameSection, RpcError},
    limits::{FRAME_PREFIX_BYTES, FrameClass},
};

fn validate_payload_length(length: usize, class: FrameClass) -> Result<u32, RpcError> {
    if length == 0 {
        return Err(RpcError::ZeroLengthFrame);
    }
    let maximum = class.max_payload_bytes();
    if length > maximum {
        return Err(RpcError::FrameTooLarge {
            declared: length,
            maximum,
        });
    }
    u32::try_from(length).map_err(|_| RpcError::FrameLengthOverflow { declared: length })
}

fn declared_payload_length(
    prefix: [u8; FRAME_PREFIX_BYTES],
    class: FrameClass,
) -> Result<usize, RpcError> {
    let declared =
        usize::try_from(u32::from_be_bytes(prefix)).map_err(|_| RpcError::FrameLengthOverflow {
            declared: usize::MAX,
        })?;
    validate_payload_length(declared, class)?;
    Ok(declared)
}

/// Encodes one already-materialized payload with the selected bounded prefix.
pub fn encode_frame(payload: &[u8], class: FrameClass) -> Result<Vec<u8>, RpcError> {
    let declared = validate_payload_length(payload.len(), class)?;
    let allocation =
        FRAME_PREFIX_BYTES
            .checked_add(payload.len())
            .ok_or(RpcError::FrameLengthOverflow {
                declared: payload.len(),
            })?;
    let mut frame = Vec::new();
    frame
        .try_reserve_exact(allocation)
        .map_err(|_| RpcError::FrameAllocationFailed {
            requested: allocation,
        })?;
    frame.extend_from_slice(&declared.to_be_bytes());
    frame.extend_from_slice(payload);
    Ok(frame)
}

/// Encodes a Prost message only after its encoded size passes the selected cap.
pub fn encode_message_frame<M: Message>(
    message: &M,
    class: FrameClass,
) -> Result<Vec<u8>, RpcError> {
    let encoded_length = message.encoded_len();
    validate_payload_length(encoded_length, class)?;
    let mut payload = Vec::new();
    payload
        .try_reserve_exact(encoded_length)
        .map_err(|_| RpcError::FrameAllocationFailed {
            requested: encoded_length,
        })?;
    message.encode(&mut payload)?;
    encode_frame(&payload, class)
}

/// Splits exact-one-frame bytes without copying and rejects any trailing frame.
pub fn decode_exact_frame(bytes: &[u8], class: FrameClass) -> Result<&[u8], RpcError> {
    if bytes.len() < FRAME_PREFIX_BYTES {
        return Err(RpcError::TruncatedFrame {
            section: FrameSection::Prefix,
            expected: FRAME_PREFIX_BYTES,
            received: bytes.len(),
        });
    }
    let mut prefix = [0_u8; FRAME_PREFIX_BYTES];
    prefix.copy_from_slice(&bytes[..FRAME_PREFIX_BYTES]);
    let declared = declared_payload_length(prefix, class)?;
    let received = bytes.len() - FRAME_PREFIX_BYTES;
    if received < declared {
        return Err(RpcError::TruncatedFrame {
            section: FrameSection::Payload,
            expected: declared,
            received,
        });
    }
    if received > declared {
        return Err(RpcError::TrailingFrameData {
            trailing: received - declared,
        });
    }
    Ok(&bytes[FRAME_PREFIX_BYTES..])
}

/// Decodes a Prost payload after exact framing has been checked.
pub fn decode_message_frame<M>(bytes: &[u8], class: FrameClass) -> Result<M, RpcError>
where
    M: Message + Default,
{
    Ok(M::decode(decode_exact_frame(bytes, class)?)?)
}

async fn read_counted<R>(
    reader: &mut R,
    target: &mut [u8],
    section: FrameSection,
) -> Result<(), RpcError>
where
    R: AsyncRead + Unpin,
{
    let mut received = 0;
    while received < target.len() {
        let count = reader.read(&mut target[received..]).await?;
        if count == 0 {
            return Err(RpcError::TruncatedFrame {
                section,
                expected: target.len(),
                received,
            });
        }
        received += count;
    }
    Ok(())
}

/// Reads one bounded frame, rejecting its length before allocating the body.
pub async fn read_frame<R>(reader: &mut R, class: FrameClass) -> Result<Vec<u8>, RpcError>
where
    R: AsyncRead + Unpin,
{
    let mut prefix = [0_u8; FRAME_PREFIX_BYTES];
    read_counted(reader, &mut prefix, FrameSection::Prefix).await?;
    let declared = declared_payload_length(prefix, class)?;

    let mut payload = Vec::new();
    payload
        .try_reserve_exact(declared)
        .map_err(|_| RpcError::FrameAllocationFailed {
            requested: declared,
        })?;
    payload.resize(declared, 0);
    read_counted(reader, &mut payload, FrameSection::Payload).await?;
    Ok(payload)
}

/// Writes one frame through any in-memory `AsyncWrite`, including fragmented writers.
pub async fn write_frame<W>(
    writer: &mut W,
    payload: &[u8],
    class: FrameClass,
) -> Result<(), RpcError>
where
    W: AsyncWrite + Unpin,
{
    let declared = validate_payload_length(payload.len(), class)?;
    writer.write_all(&declared.to_be_bytes()).await?;
    writer.write_all(payload).await?;
    Ok(())
}

/// Encodes and writes a Prost message after checking its size before allocation.
pub async fn write_message<W, M>(
    writer: &mut W,
    message: &M,
    class: FrameClass,
) -> Result<(), RpcError>
where
    W: AsyncWrite + Unpin,
    M: Message,
{
    let encoded_length = message.encoded_len();
    validate_payload_length(encoded_length, class)?;
    let mut payload = Vec::new();
    payload
        .try_reserve_exact(encoded_length)
        .map_err(|_| RpcError::FrameAllocationFailed {
            requested: encoded_length,
        })?;
    message.encode(&mut payload)?;
    write_frame(writer, &payload, class).await
}
