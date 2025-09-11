/*
 * Copyright (C) 2021 The Android Open Source Project
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

//! # The Trusty IPC Library
//!
//! Implements IPC between Trusty userspace apps, the Trusty kernel, and the
//! non-secure kernel (Android).
//!
//! This library is based around the [`Handle`] type, which is an open
//! communication channel between a client and a service. This handle can send
//! and receive messages which implement [`Serialize`] and [`Deserialize`],
//! respectively. Receiving a message blocks until a message arrives or the
//! handle is closed.

#![feature(allocator_api)]

#[allow(non_camel_case_types)]
#[allow(unused)]
mod sys {
    use peer_id::trusty_peer_id_kind_t;

    include!(env!("BINDGEN_INC_FILE"));
}

mod err;
mod handle;
pub mod raw;
mod serialization;
mod service;

pub use err::{ConnectResult, MessageResult, Result, TipcError};
pub use handle::{Handle, MMapFlags, UnsafeSharedBuf, MAX_MSG_HANDLES};
pub use peer_id::{Uuid, UuidFromBytesError, UuidFromStrError};
pub use serialization::{Deserialize, Serialize, Serializer};
pub use service::{
    ClientIdentifier, ClientIdentifierError, Dispatcher, Manager, PortCfg, Service,
    UnbufferedService,
};

// Safety:
//   * Prefix - Guaranteed by C implementation/bindgen.
//   * No Padding - Guaranteed by C implementation.
//   * len Impl - Self has statically know size.
//   * Distinct KIND - Guaranteed by C implementation.
//   * KIND Range - Guaranteed by C implementation.
unsafe impl peer_id::TrustyPeerId for sys::trusty_peer_id_sequence_id {
    const KIND: peer_id::trusty_peer_id_kind_t = sys::TRUSTY_PEER_ID_KIND_SEQUENCE_ID;

    // # Safety
    //   Always safe.
    unsafe fn len(_data: *const u8) -> usize {
        core::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test() {}

    test::init!();
}
