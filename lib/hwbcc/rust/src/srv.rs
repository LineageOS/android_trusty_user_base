/*
 * Copyright (C) 2025 The Android Open Source Project
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//! Service library for implementing hwbcc servers in Rust

use crate::{
    hwbcc_req_hdr, hwbcc_req_sign_data, BccCmd, BccMsg, HwBccError, HwBccMode, HwBccResponse,
    SigningAlgorithm, HWBCC_MAX_AAD_SIZE, HWBCC_MAX_DATA_TO_SIGN_SIZE,
};
use alloc::rc::Rc;
use core::mem::size_of;
use num_traits::FromPrimitive;
use tipc::{ConnectResult, Deserialize, Handle, MessageResult, PortCfg, Service, TipcError, Uuid};
use trusty_std::alloc::TryAllocFrom;
use trusty_sys::Error;
use zerocopy::FromBytes;

const HWBCC_MAX_MESSAGE_SIZE: usize = size_of::<hwbcc_req_hdr>()
    + size_of::<hwbcc_req_sign_data>()
    + HWBCC_MAX_AAD_SIZE as usize
    + HWBCC_MAX_DATA_TO_SIGN_SIZE as usize;

pub struct RequestContext {
    pub peer: Uuid,
}

/// A raw buffer that holds a serialized `BccMsg` and allows
/// for extraction of a `BccMsg` after deserialization.
///
/// Deserialization itself performs no validation and only
/// copies the input buffer into the `RawBccMsgBuffer`. All
/// validation occurs when calling `to_req_msg`.
///
/// This approach is needed because `BccMsg` is implemented
/// to support serializing from borrowed data in the client.
/// As such, it specifies an explicit lifetime, which is not
/// supported by the `Message` type of the `Service` trait.
pub struct RawBccMsgBuffer(Vec<u8>);

impl RawBccMsgBuffer {
    #[allow(dead_code)]
    fn to_req_msg(&self) -> Result<BccMsg<'_>, HwBccError> {
        let msg_buffer = &self.0;

        match hwbcc_req_hdr::read_from_prefix(msg_buffer) {
            Ok((header, sign_data_msg)) => {
                let mut req_msg = BccMsg::new(
                    BccCmd::from_u32(header.cmd).ok_or_else(|| HwBccError::TryFromIntError)?,
                    HwBccMode::from_u32(header.test_mode)
                        .ok_or_else(|| HwBccError::TryFromIntError)?,
                    header.context,
                );

                if sign_data_msg.len() > 0 {
                    let _ = deserialize_signing_msg(&sign_data_msg, &mut req_msg)?;
                }

                Ok(req_msg)
            }
            Err(_) => {
                log::error!("hwbcc request too small. Smaller than required header.");
                return Err(HwBccError::BadLen);
            }
        }
    }
}

impl Deserialize for RawBccMsgBuffer {
    type Error = TipcError;
    const MAX_SERIALIZED_SIZE: usize = HWBCC_MAX_MESSAGE_SIZE;

    fn deserialize(bytes: &[u8], _handles: &mut [Option<Handle>]) -> tipc::Result<Self> {
        Ok(RawBccMsgBuffer(Vec::try_alloc_from(bytes)?))
    }
}

fn deserialize_signing_msg<'a>(
    sign_data_bytes: &'a [u8],
    msg: &mut BccMsg<'a>,
) -> Result<(), HwBccError> {
    match hwbcc_req_sign_data::read_from_prefix(sign_data_bytes) {
        Ok((sign_header, rest)) => {
            let algorithm = SigningAlgorithm::from_i16(sign_header.algorithm)
                .ok_or_else(|| HwBccError::TryFromIntError)?;

            if sign_header.aad_size > HWBCC_MAX_AAD_SIZE {
                log::error!(
                    "Failed to deserialize. aad size {} is greater than max of {}",
                    sign_header.aad_size,
                    HWBCC_MAX_AAD_SIZE
                );
                return Err(HwBccError::BadLen);
            }

            if sign_header.data_size as u32 > HWBCC_MAX_DATA_TO_SIGN_SIZE {
                log::error!(
                    "Failed to deserialize. data size {} is greater than max of {}",
                    sign_header.aad_size,
                    HWBCC_MAX_AAD_SIZE
                );
                return Err(HwBccError::BadLen);
            }

            // hwbcc defines sizes with different data types, and at the end of the
            // day we need these as usize so we can use them to index into the remaining
            // data. `data_size` is u16 so it infallibly converts. `aad_size` is unfortunately
            // u32, but hwbcc defines `MAX_AAD_SIZE` as 512 bytes, which should always fit in
            // rust's usize.
            let data_size: usize = sign_header.data_size.into();
            let aad_size: usize = sign_header.aad_size.try_into()?;

            let required_rest_len = data_size + aad_size;

            if rest.len() < required_rest_len {
                log::error!("hwbcc_req_sign_data header sizes are larger than the provided buffer");
                return Err(HwBccError::BadLen);
            }

            // We've checked above to ensure that these indices are not out of range.
            // In the event that a larger buffer was sent than the configured sizes, this
            // truncates to the sizes provided in the header.
            let _ = msg.add_signing_req(
                algorithm,
                &rest[..data_size],
                &rest[data_size..required_rest_len],
            );

            Ok(())
        }
        Err(_) => {
            log::error!("hwbcc_req_sign_data request too small.");
            Err(HwBccError::BadLen)
        }
    }
}

pub struct HwBccService {
    ops: Rc<dyn HwBccOps>,
}

impl HwBccService {
    pub fn new(ops: Rc<dyn HwBccOps>) -> Self {
        Self { ops }
    }
}

pub trait HwBccOps {
    fn init(&self, session: &RequestContext) -> Result<(), HwBccError>;
    fn close(&self, session: &RequestContext);
    fn get_bcc(&self, session: &RequestContext, mode: HwBccMode) -> Result<Vec<u8>, HwBccError>;
}

impl Service for HwBccService {
    type Connection = RequestContext;
    type Message = RawBccMsgBuffer;
    fn on_connect(
        &self,
        _: &PortCfg,
        _: &Handle,
        peer: &Uuid,
    ) -> Result<ConnectResult<<Self as Service>::Connection>, TipcError> {
        log::debug!("Accepted connection from uuid {:?}.", peer);
        let session = RequestContext { peer: peer.clone() };
        match self.ops.init(&session) {
            Ok(_) => Ok(ConnectResult::Accept(session)),
            Err(e) => {
                log::error!("Failed HwBccOps.init: {:?}", e);
                Err(TipcError::UnknownError)
            }
        }
    }

    fn on_message(
        &self,
        connection: &<Self as Service>::Connection,
        handle: &Handle,
        message: RawBccMsgBuffer,
    ) -> Result<MessageResult, TipcError> {
        let bcc_msg = match message.to_req_msg() {
            Ok(m) => m,
            Err(_) => {
                log::error!("Failed to deserialize request");
                return Err(TipcError::InvalidData);
            }
        };

        let header = &bcc_msg.header;

        let response = match header.cmd {
            BccCmd::GetBcc => {
                payload_to_response(BccCmd::GetBcc, self.ops.get_bcc(connection, header.test_mode))?
            }
            _ => HwBccResponse::new_without_payload(Error::NotSupported, header.cmd),
        };

        handle.send(&response)?;

        Ok(MessageResult::MaintainConnection)
    }

    fn on_disconnect(&self, connection: &Self::Connection) {
        self.ops.close(connection);
    }
}

fn payload_to_response(
    cmd: BccCmd,
    payload: Result<Vec<u8>, HwBccError>,
) -> Result<HwBccResponse, TipcError> {
    match payload {
        Ok(p) => HwBccResponse::try_new_with_payload(Error::NoError, cmd, p).map_err(|e| {
            log::error!("Failed to create HwBccResponse from payload. Error: {:?}", e);
            TipcError::InvalidData
        }),
        Err(_) => Ok(HwBccResponse::new_without_payload(Error::Generic, cmd)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_serializer::TestSerializer;
    use test::expect_eq;
    use tipc::Serialize;

    #[test]
    fn deserialize_header_only() {
        let mut serializer = TestSerializer::default();
        let msg = BccMsg::new(BccCmd::GetBcc, HwBccMode::Test, 5);
        let _ = msg.serialize(&mut serializer).expect("serialization of BccMsg failed");

        let deserialized =
            &RawBccMsgBuffer::deserialize(&mut serializer.buffers, &mut serializer.handles)
                .expect("deserialization failed");

        expect_eq!(deserialized.to_req_msg().expect("failed to get deserialized BccMsg"), msg);
    }

    #[test]
    fn deserialize_with_sign_data() {
        let mut serializer = TestSerializer::default();
        let mut msg = BccMsg::new(BccCmd::GetBcc, HwBccMode::Test, 5);
        let data = vec![0xBB, 0xBB, 0xBB];
        let aad = vec![0xCC, 0xCC, 0xCC];
        msg.add_signing_req(SigningAlgorithm::ED25519, &data, &aad);
        let _ = msg.serialize(&mut serializer).expect("serialization of BccMsg failed");

        let deserialized =
            &RawBccMsgBuffer::deserialize(&mut serializer.buffers, &mut serializer.handles)
                .expect("deserialization failed");

        expect_eq!(deserialized.to_req_msg().expect("failed to get deserialized BccMsg"), msg);
    }

    #[test]
    fn deserialize_fail_if_main_header_too_small() {
        let mut serializer = TestSerializer::default();
        serializer.buffers.resize(size_of::<hwbcc_req_hdr>() - 1, 0xAA);
        let deserialized =
            &RawBccMsgBuffer::deserialize(&mut serializer.buffers, &mut serializer.handles)
                .expect("deserialization failed");

        expect_eq!(deserialized.to_req_msg().err(), Some(HwBccError::BadLen));
    }

    #[test]
    fn deserialize_fail_if_sign_header_too_small() {
        let mut serializer = TestSerializer::default();
        let msg = BccMsg::new(BccCmd::GetBcc, HwBccMode::Test, 5);
        let _ = msg.serialize(&mut serializer).expect("serialization of BccMsg failed");

        // The signing header is encoded directly after the main header so this results in
        // a signing header of length 1.
        serializer.buffers.push(0xAA);
        let deserialized =
            &RawBccMsgBuffer::deserialize(&mut serializer.buffers, &mut serializer.handles)
                .expect("deserialization failed");

        expect_eq!(deserialized.to_req_msg().err(), Some(HwBccError::BadLen));
    }

    #[test]
    fn deserialize_with_sign_req_should_truncate_buffer() {
        let mut serializer = TestSerializer::default();
        let mut msg = BccMsg::new(BccCmd::GetBcc, HwBccMode::Test, 5);
        msg.add_signing_req(SigningAlgorithm::ED25519, b"signing data", b"aad data");
        let _ = msg.serialize(&mut serializer).expect("serialization of BccMsg failed");

        // The end of a message is the AAD data. We're adding erroneous data on the end here
        // and ensuring that when deserializing, it's ignored.
        serializer.buffers.push(0xAA);
        let deserialized =
            &RawBccMsgBuffer::deserialize(&mut serializer.buffers, &mut serializer.handles)
                .expect("deserialization failed");

        expect_eq!(deserialized.to_req_msg().expect("failed to get deserialized BccMsg"), msg);
    }
}
