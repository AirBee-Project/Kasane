use bytes::{Buf, BufMut, Bytes, BytesMut};
use prost::Message;
use thiserror::Error;

pub const GRPC_HEADER_SIZE: usize = 5;

#[derive(Debug, Error)]
pub enum GrpcFrameError {
    #[error("Incomplete gRPC frame: expected {expected} bytes, got {actual} bytes")]
    IncompleteFrame { expected: usize, actual: usize },
    #[error("Message length {0} exceeds maximum allowed size")]
    MessageTooLarge(usize),
    #[error("Protobuf decode error: {0}")]
    DecodeError(#[from] prost::DecodeError),
    #[error("Protobuf encode error: {0}")]
    EncodeError(#[from] prost::EncodeError),
}

/// 単一の Protobuf メッセージを 5 バイトの gRPC フレームヘッダ付きでエンコード
pub fn encode_grpc_frame<T: Message>(msg: &T) -> Result<Bytes, GrpcFrameError> {
    let msg_len = msg.encoded_len();
    let mut buf = BytesMut::with_capacity(GRPC_HEADER_SIZE + msg_len);

    // 圧縮フラグ: 0 (非圧縮)
    buf.put_u8(0);
    // メッセージ長: u32 big-endian
    buf.put_u32(msg_len as u32);

    msg.encode(&mut buf)?;
    Ok(buf.freeze())
}

/// 単一の gRPC フレームバイト列からメッセージをデコード
pub fn decode_grpc_frame<T: Message + Default>(mut buf: &[u8]) -> Result<T, GrpcFrameError> {
    if buf.len() < GRPC_HEADER_SIZE {
        return Err(GrpcFrameError::IncompleteFrame {
            expected: GRPC_HEADER_SIZE,
            actual: buf.len(),
        });
    }

    let _compressed = buf.get_u8();
    let msg_len = buf.get_u32() as usize;

    if buf.len() < msg_len {
        return Err(GrpcFrameError::IncompleteFrame {
            expected: msg_len,
            actual: buf.len(),
        });
    }

    let msg = T::decode(&buf[..msg_len])?;
    Ok(msg)
}

/// 連続したバイト列からすべての gRPC メッセージを抽出・デコード
pub fn decode_all_grpc_frames<T: Message + Default>(
    mut data: &[u8],
) -> Result<Vec<T>, GrpcFrameError> {
    let mut messages = Vec::new();

    while !data.is_empty() {
        if data.len() < GRPC_HEADER_SIZE {
            return Err(GrpcFrameError::IncompleteFrame {
                expected: GRPC_HEADER_SIZE,
                actual: data.len(),
            });
        }

        let _compressed = data[0];
        let msg_len = u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as usize;

        if data.len() < GRPC_HEADER_SIZE + msg_len {
            return Err(GrpcFrameError::IncompleteFrame {
                expected: GRPC_HEADER_SIZE + msg_len,
                actual: data.len(),
            });
        }

        let msg = T::decode(&data[GRPC_HEADER_SIZE..GRPC_HEADER_SIZE + msg_len])?;
        messages.push(msg);
        data = &data[GRPC_HEADER_SIZE + msg_len..];
    }

    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kasane_proto::kasane::SearchDataResponse;

    #[test]
    fn test_encode_and_decode_grpc_frame() {
        let original = SearchDataResponse {
            dictionary: vec![],
            data: vec![],
        };

        let frame = encode_grpc_frame(&original).expect("encode succeeds");
        assert_eq!(frame.len(), GRPC_HEADER_SIZE);
        assert_eq!(frame[0], 0); // uncompressed
        assert_eq!(&frame[1..5], &[0, 0, 0, 0]); // 0 length

        let decoded: SearchDataResponse =
            decode_grpc_frame(&frame).expect("decode succeeds");
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_decode_all_grpc_frames() {
        let resp1 = SearchDataResponse {
            dictionary: vec![],
            data: vec![],
        };
        let resp2 = SearchDataResponse {
            dictionary: vec![],
            data: vec![],
        };

        let frame1 = encode_grpc_frame(&resp1).unwrap();
        let frame2 = encode_grpc_frame(&resp2).unwrap();

        let mut combined = Vec::new();
        combined.extend_from_slice(&frame1);
        combined.extend_from_slice(&frame2);

        let decoded_all: Vec<SearchDataResponse> =
            decode_all_grpc_frames(&combined).expect("decode all succeeds");
        assert_eq!(decoded_all.len(), 2);
    }
}
