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

use alloc::rc::{Rc, Weak};
use core::fmt;
use core::mem::{ManuallyDrop, MaybeUninit};
use log::{debug, error, warn};
use peer_id::Uuid;
use trusty_std::alloc::{AllocError, Vec};
use trusty_std::ffi::{CString, FallibleCString};
use trusty_std::TryClone;
use trusty_sys::c_void;

use crate::handle::MAX_MSG_HANDLES;
use crate::sys;
use crate::{ConnectResult, Deserialize, Handle, MessageResult, Result, TipcError};
use handle_set::HandleSet;
mod handle_set;

/// A description of a server-side IPC port.
///
/// A port configuration specifies the service port and various parameters for
/// the service. This configuration struct is a builder to set these parameters.
///
/// # Examples
///
/// ```
/// # impl Service for () {
/// #     type Connection = ();
/// #     type Message = ();
///
/// #     fn on_connect(
/// #         &self,
/// #         _port: &PortCfg,
/// #         _handle: &Handle,
/// #         _peer: &Uuid,
/// #     ) -> Result<ConnectResult<Self::Connection>> {
/// #         Ok(ConnectResult::Accept(()))
/// #     }
/// #
/// #     fn on_message(
/// #         &self,
/// #         _connection: &Self::Connection,
/// #         _handle: &Handle,
/// #         _msg: Self::Message,
/// #     ) -> Result<MessageResult> {
/// #         Ok(MessageResult::MaintainConnection)
/// #     }
/// # }
///
/// let cfg = PortCfg::new("com.android.trusty.rust_port_test")
///     .msg_queue_len(4)
///     .msg_max_size(4096)
///     .allow_ta_connect();
///
/// let service = ();
/// let buffer = [0u8; 4096];
/// let manager = Manager::new(service, cfg, buffer);
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct PortCfg {
    path: CString,
    msg_queue_len: u32,
    msg_max_size: u32,
    flags: u32,
    uuid_allow_list: Option<&'static [Uuid]>,
}

impl PortCfg {
    /// Construct a new port configuration for the given path
    pub fn new<T: AsRef<str>>(path: T) -> Result<Self> {
        Ok(Self {
            path: CString::try_new(path.as_ref())?,
            msg_queue_len: 1,
            msg_max_size: 4096,
            flags: 0,
            uuid_allow_list: None,
        })
    }

    /// Construct a new port configuration for the given path
    ///
    /// This version takes ownership of the path and does not allocate.
    pub fn new_raw(path: CString) -> Self {
        Self { path, msg_queue_len: 1, msg_max_size: 4096, flags: 0, uuid_allow_list: None }
    }

    /// Set the message queue length for this port configuration
    pub fn msg_queue_len(self, msg_queue_len: u32) -> Self {
        Self { msg_queue_len, ..self }
    }

    /// Set the message maximum length for this port configuration
    pub fn msg_max_size(self, msg_max_size: u32) -> Self {
        Self { msg_max_size, ..self }
    }

    /// Allow connections from non-secure (Android) clients for this port
    /// configuration
    pub fn allow_ns_connect(self) -> Self {
        Self { flags: self.flags | sys::IPC_PORT_ALLOW_NS_CONNECT as u32, ..self }
    }

    /// Allow connections from secure (Trusty) client for this port
    /// configuration
    pub fn allow_ta_connect(self) -> Self {
        Self { flags: self.flags | sys::IPC_PORT_ALLOW_TA_CONNECT as u32, ..self }
    }

    /// Filter allowable UUID connections. Leaving this unset will allow connection from any peer
    /// UUID. Services should handle any additional filtering they need.
    pub fn allowed_uuids(self, uuids: &'static [Uuid]) -> Self {
        Self { uuid_allow_list: Some(uuids), ..self }
    }

    /// Get path
    pub fn get_path(&self) -> &CString {
        &self.path
    }

    /// Get message max size
    pub fn get_msg_max_size(&self) -> u32 {
        self.msg_max_size
    }

    /// Get message queue length
    pub fn get_msg_queue_len(&self) -> u32 {
        self.msg_queue_len
    }

    /// Get flags
    pub fn get_flags(&self) -> u32 {
        self.flags
    }

    /// Get allowed UUIDs
    pub fn get_uuid_allow_list(&self) -> Option<&'static [Uuid]> {
        self.uuid_allow_list
    }
}

impl TryClone for PortCfg {
    type Error = AllocError;

    fn try_clone(&self) -> core::result::Result<Self, Self::Error> {
        Ok(Self { path: self.path.try_clone()?, ..*self })
    }
}

pub(crate) struct Channel<D: Dispatcher> {
    handle: Handle,
    ty: ChannelTy<D>,
}

impl<D: Dispatcher> PartialEq for Channel<D> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}

impl<D: Dispatcher> Eq for Channel<D> {}

impl<D: Dispatcher> fmt::Debug for Channel<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Channel {{")?;
        writeln!(f, "  handle: {:?},", self.handle)?;
        writeln!(f, "  ty: {:?},", self.ty)?;
        write!(f, "}}")
    }
}

enum ChannelTy<D: Dispatcher> {
    /// Service port with a configuration describing the port
    Port(PortCfg),

    /// Client connection
    Connection(D::Connection),
}

impl<D: Dispatcher> fmt::Debug for ChannelTy<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChannelTy::Port(cfg) => write!(f, "ChannelTy::Port({:?})", cfg),
            ChannelTy::Connection(_) => write!(f, "ChannelTy::Connection"),
        }
    }
}

impl<D: Dispatcher> Channel<D> {
    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    pub fn is_port(&self) -> bool {
        match self.ty {
            ChannelTy::Port(..) => true,
            _ => false,
        }
    }

    pub fn is_connection(&self) -> bool {
        match self.ty {
            ChannelTy::Connection(..) => true,
            _ => false,
        }
    }

    /// Reconstruct a reference to this type from an opaque pointer.
    ///
    /// SAFETY: The opaque pointer must have been constructed using
    /// Weak::into_raw, which happens in HandleSet::do_set_ctrl to create a
    /// connection cookie.
    unsafe fn from_opaque_ptr<'a>(ptr: *const c_void) -> Option<Rc<Self>> {
        if ptr.is_null() {
            None
        } else {
            // We must not drop the weak pointer here, because we are not
            // actually taking ownership of it.
            let weak = ManuallyDrop::new(Weak::from_raw(ptr.cast()));
            weak.upgrade()
        }
    }

    pub(crate) fn try_new_port(cfg: &PortCfg) -> Result<Rc<Self>> {
        // SAFETY: syscall, config path is borrowed and outlives the call.
        // Return value is either a negative error code or a valid handle.
        let rc = unsafe {
            trusty_sys::port_create(
                cfg.path.as_ptr(),
                cfg.msg_queue_len,
                cfg.msg_max_size,
                cfg.flags,
            )
        };
        if rc < 0 {
            Err(TipcError::from_uapi(rc))
        } else {
            Ok(Rc::try_new(Self {
                handle: Handle::from_raw(rc as i32)?,
                ty: ChannelTy::Port(cfg.try_clone()?),
            })?)
        }
    }

    fn try_new_connection(handle: Handle, data: D::Connection) -> Result<Rc<Self>> {
        Ok(Rc::try_new(Self { handle, ty: ChannelTy::Connection(data) })?)
    }
}

/// Enum identifying a VM client of a Trusty service.
///
/// Non-exhaustive because we want to have to ability to add more kinds in the future.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VmIdentifier {
    Ffa(u16),
}

/// Error enum for [`ClientIdentifier::try_from_c_repr`]
#[derive(Debug)]
pub enum ClientIdentifierError {
    /// The C representation had an invalid kind value.
    InvalidKind,
    /// Encountered an error trying to make the C repr into a concrete error type to match its kind.
    AsConcreteError(trusty_sys::AsConcreteError),
    /// The C representation had a kind value which we don't know how to convert.
    UnknownKind(trusty_sys::trusty_peer_id_kind_t),
}

impl core::fmt::Display for ClientIdentifierError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Self::InvalidKind => write!(
                f,
                "Got uninitialized/zeroed trusty_peer_id. Did you forget to \
                    initialize it?"
            ),
            Self::AsConcreteError(e) => {
                write!(f, "Failed to convert to concrete peer ID type {:?}", e)
            }
            Self::UnknownKind(kind) => write!(f, "Unknown trusty_peer_id kind {}", kind),
        }
    }
}

impl From<trusty_sys::AsConcreteError> for ClientIdentifierError {
    fn from(e: trusty_sys::AsConcreteError) -> Self {
        Self::AsConcreteError(e)
    }
}

/// Enum identifying the client of a Trusty service.
///
/// Non-exhaustive because we want to have to ability to add more kinds in the future.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientIdentifier {
    /// The identifier of a local client TA that connects to the service
    UUID(Uuid),

    /// The identifier of a remote client TA that connects to the service by AuthMgr-BE
    /// This introduction happens via an AIDL interface. Currently, AIDL types do not support u64.
    /// Therefore, we use i64 here, although client sequence number cannot be negative.
    ClientSeqNumber(i64),

    /// The identifier of a remote pVM that connects to the AuthMgr-BE TA. This identifier type is
    /// primarily used during the AuthMgr protocol. In the legitimate settings, only the AuthMgr-FE
    /// TA in the remote pVM connects to the AuthMgr-BE TA.
    VMID(VmIdentifier),
}

impl ClientIdentifier {
    /// Convert `self` into a `trusty_peer_id`, so that it can be passed via FFI to be read by C
    /// code.
    ///
    /// This allows, for example, a Rust binder RpcServer, with a per-session callback written in C.
    pub fn c_repr(&self) -> trusty_sys::TrustyPeerIdStorageSized {
        match self {
            Self::UUID(uuid) => trusty_sys::TrustyPeerIdStorageSized::from_concrete(
                &trusty_sys::trusty_peer_id_uuid {
                    kind: trusty_sys::TRUSTY_PEER_ID_KIND_UUID,
                    id: *uuid.as_ref(),
                },
            ),
            Self::VMID(VmIdentifier::Ffa(vm_id)) => {
                trusty_sys::TrustyPeerIdStorageSized::from_concrete(
                    &trusty_sys::trusty_peer_id_vmid_ffa {
                        kind: trusty_sys::TRUSTY_PEER_ID_KIND_VMID_FFA,
                        reserved_1: 1, // Mandated by doccomment in trusty_peer_id.h
                        id: (*vm_id).into(),
                        padding: Default::default(),
                    },
                )
            }
            Self::ClientSeqNumber(sequence_num) => {
                trusty_sys::TrustyPeerIdStorageSized::from_concrete(
                    &sys::trusty_peer_id_sequence_id {
                        kind: sys::TRUSTY_PEER_ID_KIND_SEQUENCE_ID,
                        id: *sequence_num,
                    },
                )
            }
        }
    }

    /// Tries to convert a `trusty_peer_id` into a ClientIdentifier. See `ClientIdentifierError` for
    /// reasons this might fail.
    pub fn try_from_c_repr(
        id: trusty_sys::TrustyPeerIdRef<'_>,
    ) -> core::result::Result<Self, ClientIdentifierError> {
        match id.kind() {
            trusty_sys::TRUSTY_PEER_ID_KIND_INVALID => Err(ClientIdentifierError::InvalidKind),
            trusty_sys::TRUSTY_PEER_ID_KIND_UUID => {
                let uuid = id.try_as_concrete::<trusty_sys::trusty_peer_id_uuid>()?;
                Ok(Self::UUID(Uuid::from(uuid.id)))
            }
            trusty_sys::TRUSTY_PEER_ID_KIND_VMID_FFA => {
                let vmid = id.try_as_concrete::<trusty_sys::trusty_peer_id_vmid_ffa>()?;
                assert_eq!(vmid.reserved_1, 1);
                vmid.padding.iter().for_each(|x| assert_eq!(*x, 0));
                Ok(Self::VMID(VmIdentifier::Ffa(vmid.id)))
            }
            sys::TRUSTY_PEER_ID_KIND_SEQUENCE_ID => {
                let seqid = id.try_as_concrete::<sys::trusty_peer_id_sequence_id>()?;
                Ok(Self::ClientSeqNumber(seqid.id))
            }
            _ => Err(ClientIdentifierError::UnknownKind(id.kind())),
        }
    }

    /// Convert a `trusty_peer_id` into a ClientIdentifier.
    ///
    /// # Panics
    ///
    ///   If the conversion fails. This probably means you're reading an uninitialized
    ///   `trusty_peer_id`, or that you need to add a variant to the ClientIdentifier enum.
    pub fn from_c_repr(id: trusty_sys::TrustyPeerIdRef<'_>) -> Self {
        Self::try_from_c_repr(id).expect("Conversion failure due to programmer error.")
    }
}

/// A service which handles IPC messages for a collection of ports.
///
/// A service which implements this interface can register itself, along with a
/// set of ports it handles, with a [`Manager`] which then dispatches
/// connection and message events to this service.
pub trait Service {
    /// Generic type to association with a connection. `on_connect()` should
    /// create this type for a successful connection.
    type Connection;

    /// Type of message this service can receive.
    type Message: Deserialize;

    /// Called when a client connects
    ///
    /// Returns either `Ok(Accept(Connection))` if the connection should be
    /// accepted or `Ok(CloseConnection)` if the connection should be closed.
    fn on_connect(
        &self,
        port: &PortCfg,
        handle: &Handle,
        peer: &Uuid,
    ) -> Result<ConnectResult<Self::Connection>>;

    /// Called when the service receives a message.
    ///
    /// The service manager handles deserializing the message, which is then
    /// passed to this callback.
    ///
    /// Should return `Ok(MaintainConnection)` if the connection should be kept open. The
    /// connection will be closed if `Ok(CloseConnection)` or `Err(_)` is returned.
    fn on_message(
        &self,
        connection: &Self::Connection,
        handle: &Handle,
        msg: Self::Message,
    ) -> Result<MessageResult>;

    /// Called when the client closes a connection.
    fn on_disconnect(&self, _connection: &Self::Connection) {}
}

pub trait UnbufferedService {
    /// Generic type to association with a connection. `on_connect()` should
    /// create this type for a successful connection.
    type Connection;

    /// Called when a client connects
    ///
    /// Returns either `Ok(Accept(Connection))` if the connection should be
    /// accepted or `Ok(CloseConnection)` if the connection should be closed.
    fn on_connect(
        &self,
        port: &PortCfg,
        handle: &Handle,
        peer: &Uuid,
    ) -> Result<ConnectResult<Self::Connection>>;

    /// Called when the service receives a message.
    ///
    /// The service is responsible for deserializing the message.
    /// A default implementation is provided that panics, for reasons of backwards
    /// compatibility with existing code. Any unbuffered service should implement this
    /// method and also provide a simple implementation for `on_message` that e.g. logs
    /// or panics.
    ///
    /// Should return `Ok(MaintainConnection)` if the connection should be kept open. The
    /// connection will be closed if `Ok(CloseConnection)` or `Err(_)` is returned.
    fn on_message(
        &self,
        connection: &Self::Connection,
        handle: &Handle,
        buffer: &mut [u8],
    ) -> Result<MessageResult>;

    /// Called when the client closes a connection.
    fn on_disconnect(&self, _connection: &Self::Connection) {}

    /// Get the maximum possible length of any message handled by this service
    fn max_message_length(&self) -> usize {
        0
    }

    /// Called when a client connects, where the client can be identified by either of the
    /// identifiers defined in the three variations of the `ClientIdentifier` enum.
    ///
    /// A default implementation is provided for backward compatibility. Any new service should
    /// override this method and handle the three client variations.
    fn on_new_connection(
        &self,
        port: &PortCfg,
        handle: &Handle,
        client_identifier: &ClientIdentifier,
    ) -> Result<ConnectResult<Self::Connection>> {
        match client_identifier {
            ClientIdentifier::UUID(peer) => self.on_connect(port, handle, &peer),
            _ => {
                unimplemented!();
            }
        }
    }
}

impl<T, U: Deserialize, V: Service<Connection = T, Message = U>> UnbufferedService for V {
    type Connection = <Self as Service>::Connection;

    fn on_connect(
        &self,
        port: &PortCfg,
        handle: &Handle,
        peer: &Uuid,
    ) -> Result<ConnectResult<<Self as UnbufferedService>::Connection>> {
        <Self as Service>::on_connect(self, port, handle, peer)
    }

    fn on_message(
        &self,
        connection: &<Self as UnbufferedService>::Connection,
        handle: &Handle,
        buffer: &mut [u8],
    ) -> Result<MessageResult> {
        let mut handles: [Option<Handle>; MAX_MSG_HANDLES] = Default::default();
        let (byte_count, handle_count) = handle.recv_vectored(&mut [buffer], &mut handles)?;
        let msg = <Self as Service>::Message::deserialize(
            &buffer[..byte_count],
            &mut handles[..handle_count],
        )
        .map_err(|e| {
            error!("Could not parse message: {:?}", e);
            TipcError::InvalidData
        })?;
        <Self as Service>::on_message(self, connection, handle, msg)
    }

    fn on_disconnect(&self, connection: &<Self as UnbufferedService>::Connection) {
        <Self as Service>::on_disconnect(self, connection)
    }

    fn max_message_length(&self) -> usize {
        <Self as Service>::Message::MAX_SERIALIZED_SIZE
    }
}

/// Wrap a service in a newtype.
///
/// This macro wraps an existing service in a newtype
/// that forwards the service trait implementation
/// (either [`Service`] or [`UnbufferedService`]) to
/// the wrapped service. This is useful when passing the
/// same service multiple times to the [`service_dispatcher!`]
/// macro, which requires that all the service types are distinct.
/// The wrapper(s) can be used to serve multiple ports using
/// the same service implementation.
///
/// [`service_dispatcher!`]: crate::service_dispatcher
///
/// # Examples
///
/// ```
/// // Create a new Service2 type that wraps Service1
/// wrap_service!(Service2(Service1: Service));
/// service_dispatcher! {
///     enum ServiceDispatcher {
///         Service1,
///         Service2,
///     }
/// }
/// ```
#[macro_export]
macro_rules! wrap_service {
    ($vis:vis $wrapper:ident ($inner:ty: Service)) => {
        $crate::wrap_service!(@common $vis $wrapper $inner);
        $crate::wrap_service!(@buffered $wrapper $inner);
    };

    ($vis:vis $wrapper:ident ($inner:ty: UnbufferedService)) => {
        $crate::wrap_service!(@common $vis $wrapper $inner);
        $crate::wrap_service!(@unbuffered $wrapper $inner);
    };

    (@common $vis:vis $wrapper:ident $inner:ty) => {
        $vis struct $wrapper($inner);

        #[allow(dead_code)] // These might not be used by anything
        impl $wrapper {
            $vis fn new(inner: $inner) -> Self { Self(inner) }
            $vis fn inner(&self) -> &$inner { &self.0 }
            $vis fn inner_mut(&mut self) -> &mut $inner { &mut self.0 }
        }
    };

    (@buffered $wrapper:ident $inner:ty) => {
        impl $crate::Service for $wrapper {
            type Connection = <$inner as $crate::Service>::Connection;
            type Message = <$inner as $crate::Service>::Message;

            fn on_connect(
                &self,
                port: &$crate::PortCfg,
                handle: &$crate::Handle,
                peer: &$crate::Uuid,
            ) -> $crate::Result<$crate::ConnectResult<Self::Connection>> {
                <$inner as $crate::Service>::on_connect(&self.0, port, handle, peer)
            }

            fn on_message(
                &self,
                connection: &Self::Connection,
                handle: &$crate::Handle,
                msg: Self::Message,
            ) -> $crate::Result<$crate::MessageResult> {
                <$inner as $crate::Service>::on_message(&self.0, connection, handle, msg)
            }

            fn on_disconnect(&self, connection: &Self::Connection) {
                <$inner as $crate::Service>::on_disconnect(&self.0, connection)
            }
        }
    };

    (@unbuffered $wrapper:ident $inner:ty) => {
        impl $crate::UnbufferedService for $wrapper {
            type Connection = <$inner as $crate::UnbufferedService>::Connection;

            fn on_connect(
                &self,
                port: &$crate::PortCfg,
                handle: &$crate::Handle,
                peer: &$crate::Uuid,
            ) -> $crate::Result<$crate::ConnectResult<Self::Connection>> {
                <$inner as $crate::UnbufferedService>::on_connect(&self.0, port, handle, peer)
            }

            fn on_message(
                &self,
                connection: &Self::Connection,
                handle: &$crate::Handle,
                buffer: &mut [u8],
            ) -> $crate::Result<$crate::MessageResult> {
                <$inner as $crate::UnbufferedService>::on_message(&self.0, connection, handle, buffer)
            }

            fn on_disconnect(&self, connection: &Self::Connection) {
                <$inner as $crate::UnbufferedService>::on_disconnect(&self.0, connection)
            }

            fn max_message_length(&self) -> usize {
                <$inner as $crate::UnbufferedService>::max_message_length(&self.0)
            }
        }
    };
}

pub trait Dispatcher {
    /// Generic type to association with a connection. `on_connect()` should
    /// create this type for a successful connection.
    type Connection;

    /// Called when a client connects
    ///
    /// Returns either `Ok(Accept(Connection))` if the connection should be
    /// accepted or `Ok(CloseConnection)` if the connection should be closed.
    fn on_connect(
        &self,
        port: &PortCfg,
        handle: &Handle,
        peer: &Uuid,
    ) -> Result<ConnectResult<Self::Connection>>;

    /// Called when the service receives a message.
    ///
    /// The service manager handles deserializing the message, which is then
    /// passed to this callback.
    ///
    /// Should return `Ok(MaintainConnection)` if the connection should be kept open. The
    /// connection will be closed if `Ok(CloseConnection)` or `Err(_)` is returned.
    fn on_message(
        &self,
        connection: &Self::Connection,
        handle: &Handle,
        buffer: &mut [u8],
    ) -> Result<MessageResult>;

    /// Called when the client closes a connection.
    fn on_disconnect(&self, _connection: &Self::Connection) {}

    /// Get the list of ports this dispatcher handles.
    fn port_configurations(&self) -> &[PortCfg];

    /// Get the maximum possible length of any message handled by this
    /// dispatcher.
    fn max_message_length(&self) -> usize;
}

// Implementation of a static dispatcher for services with only a single message
// type.
pub struct SingleDispatcher<S: Service> {
    service: S,
    ports: [PortCfg; 1],
}

impl<S: Service> SingleDispatcher<S> {
    fn new(service: S, port: PortCfg) -> Self {
        Self { service, ports: [port] }
    }
}

impl<S: Service> Dispatcher for SingleDispatcher<S> {
    type Connection = S::Connection;

    fn on_connect(
        &self,
        port: &PortCfg,
        handle: &Handle,
        peer: &Uuid,
    ) -> Result<ConnectResult<Self::Connection>> {
        self.service.on_connect(port, handle, peer)
    }

    fn on_message(
        &self,
        connection: &Self::Connection,
        handle: &Handle,
        buffer: &mut [u8],
    ) -> Result<MessageResult> {
        let mut handles: [Option<Handle>; MAX_MSG_HANDLES] = Default::default();
        let (byte_count, handle_count) = handle.recv_vectored(&mut [buffer], &mut handles)?;
        let msg = S::Message::deserialize(&buffer[..byte_count], &mut handles[..handle_count])
            .map_err(|e| {
                error!("Could not parse message: {:?}", e);
                TipcError::InvalidData
            })?;
        self.service.on_message(connection, handle, msg)
    }

    fn on_disconnect(&self, connection: &Self::Connection) {
        self.service.on_disconnect(connection)
    }

    fn max_message_length(&self) -> usize {
        S::Message::MAX_SERIALIZED_SIZE
    }

    fn port_configurations(&self) -> &[PortCfg] {
        &self.ports
    }
}

// Implementation of a static dispatcher for unbuffered services.
pub struct SingleUnbufferedDispatcher<S: UnbufferedService> {
    service: S,
    ports: [PortCfg; 1],
}

impl<S: UnbufferedService> SingleUnbufferedDispatcher<S> {
    fn new(service: S, port: PortCfg) -> Self {
        Self { service, ports: [port] }
    }
}

impl<S: UnbufferedService> Dispatcher for SingleUnbufferedDispatcher<S> {
    type Connection = S::Connection;

    fn on_connect(
        &self,
        port: &PortCfg,
        handle: &Handle,
        peer: &Uuid,
    ) -> Result<ConnectResult<Self::Connection>> {
        self.service.on_connect(port, handle, peer)
    }

    fn on_message(
        &self,
        connection: &Self::Connection,
        handle: &Handle,
        buffer: &mut [u8],
    ) -> Result<MessageResult> {
        self.service.on_message(connection, handle, buffer)
    }

    fn on_disconnect(&self, connection: &Self::Connection) {
        self.service.on_disconnect(connection)
    }

    fn max_message_length(&self) -> usize {
        self.service.max_message_length()
    }

    fn port_configurations(&self) -> &[PortCfg] {
        &self.ports
    }
}

/// Create a new service dispatcher that can handle a specified set of service
/// types.
///
/// This macro creates a multi-service dispatcher that holds different types of
/// services that should share the same event loop and dispatches to the
/// relevant service based on the connection port. Services must implement the
/// [`Service`] trait. An instance of this dispatcher must have a single const
/// usize parameter specifying how many ports the dispatcher will handle.
/// This macro has limited lifetime support. A single lifetime can be
/// used for the ServiceDispatcher enum and the included services (see the
/// supported definition in the Examples section).
///
/// # Examples
/// ```
/// service_dispatcher! {
///     enum ServiceDispatcher {
///         Service1,
///         Service2,
///     }
/// }
///
/// // Create a new dispatcher that handles two ports
/// let dispatcher = ServiceDispatcher::<2>::new()
///     .expect("Could not allocate service dispatcher");
///
/// let cfg = PortCfg::new(&"com.android.trusty.test_port1).unwrap();
/// dispatcher.add_service(Rc::new(Service1), cfg).expect("Could not add service 1");
///
/// let cfg = PortCfg::new(&"com.android.trusty.test_port2).unwrap();
/// dispatcher.add_service(Rc::new(Service2), cfg).expect("Could not add service 2");
/// ```
///
/// ## defining a dispatcher with multiple lifetimes
/// ```
/// service_dispatcher! {
///     enum ServiceDispatcher<'a> {
///         Service1<'a>,
///         Service2<'a>,
///     }
/// }
/// ```
#[macro_export]
macro_rules! service_dispatcher {
    ($vis:vis enum $name:ident $(<$elt: lifetime>)? {$($service:ident $(<$slt: lifetime>)? ),+ $(,)*}) => {
        /// Dispatcher that routes incoming messages to the correct server based on what
        /// port the message was sent to.
        ///
        /// This dispatcher adapts multiple different server types that expect different
        /// message formats for the same [`Manager`]. By using this dispatcher,
        /// different servers can be bound to different ports using the same event loop
        /// in the manager.
        $vis struct $name<$($elt,)? const PORT_COUNT: usize> {
            ports: arrayvec::ArrayVec::<$crate::PortCfg, PORT_COUNT>,
            services: arrayvec::ArrayVec::<ServiceKind$(<$elt>)?, PORT_COUNT>,
        }

        impl<$($elt,)? const PORT_COUNT: usize> $name<$($elt,)? PORT_COUNT> {
            pub fn new() -> $crate::Result<Self> {
                Ok(Self {
                    ports: arrayvec::ArrayVec::<_, PORT_COUNT>::new(),
                    services: arrayvec::ArrayVec::<_, PORT_COUNT>::new(),
                })
            }

            pub fn add_service<T>(&mut self, service: alloc::rc::Rc<T>, port: $crate::PortCfg) -> $crate::Result<()>
            where ServiceKind$(<$elt>)? : From<alloc::rc::Rc<T>> {
                if self.ports.is_full() || self.services.is_full() {
                    return Err($crate::TipcError::OutOfBounds);
                }
                // SAFETY: We check the size above
                unsafe {
                    self.ports.push_unchecked(port);
                    self.services.push_unchecked(service.into());
                }
                Ok(())
            }
        }

        $vis enum ServiceKind$(<$elt>)? {
            $($service(alloc::rc::Rc<$service$(<$slt>)?>)),+
        }

        $(
            impl$(<$slt>)? From<alloc::rc::Rc<$service$(<$slt>)?>> for ServiceKind$(<$slt>)? {
                fn from(service: alloc::rc::Rc<$service$(<$slt>)?>) -> Self {
                    ServiceKind::$service(service)
                }
            }
        )+

        $vis enum ConnectionKind$(<$elt>)?  {
            $($service(<$service$(<$slt>)? as $crate::UnbufferedService>::Connection)),+
        }

        impl<$($elt,)? const PORT_COUNT: usize> $crate::Dispatcher for $name<$($elt,)? PORT_COUNT> {
            type Connection = (usize, ConnectionKind$(<$elt>)?);

            fn on_connect(
                &self,
                port: &$crate::PortCfg,
                handle: &$crate::Handle,
                peer: &$crate::Uuid,
            ) -> $crate::Result<$crate::ConnectResult<Self::Connection>> {
                let port_idx = self.ports.iter()
                                         .position(|cfg| cfg == port)
                                         .ok_or($crate::TipcError::InvalidPort)?;

                match &self.services[port_idx] {
                    $(ServiceKind::$service(s) => {
                        $crate::UnbufferedService::on_connect(&**s, port, handle, peer)
                            .map(|c| c.map(|c| (port_idx, ConnectionKind::$service(c))))
                    })+
                }
            }

            fn on_message(
                &self,
                connection: &Self::Connection,
                handle: &$crate::Handle,
                buffer: &mut [u8],
            ) -> $crate::Result<$crate::MessageResult> {
                match &self.services[connection.0] {
                    $(ServiceKind::$service(s) => {
                        if let ConnectionKind::$service(conn) = &connection.1 {
                            $crate::UnbufferedService::on_message(&**s, conn, handle, buffer)
                        } else {
                            Err($crate::TipcError::InvalidData)
                        }
                    })*
                }
            }

            fn on_disconnect(&self, connection: &Self::Connection) {
                match &self.services[connection.0] {
                    $(ServiceKind::$service(s) => {
                        if let ConnectionKind::$service(conn) = &connection.1 {
                            $crate::UnbufferedService::on_disconnect(&**s, conn)
                        }
                    })*
                }
            }

            fn port_configurations(&self) -> &[$crate::PortCfg] {
                self.ports.as_slice()
            }

            fn max_message_length(&self) -> usize {
                self.services.iter().map(|s| {
                    match s {
                        $(ServiceKind::$service(service) => {
                            <$service as $crate::UnbufferedService>::max_message_length(&**service)
                        })+
                    }
                }).max().unwrap_or(0usize)
            }
        }
    };

    (@make_none $service:ident) => { None };
}

/// A manager that handles the IPC event loop and dispatches connections and
/// messages to a generic service.
pub struct Manager<
    D: Dispatcher,
    B: AsMut<[u8]> + AsRef<[u8]>,
    const PORT_COUNT: usize,
    const MAX_CONNECTION_COUNT: usize,
> {
    dispatcher: D,
    handle_set: HandleSet<D, PORT_COUNT, MAX_CONNECTION_COUNT>,
    buffer: B,
}

impl<
        S: Service,
        B: AsMut<[u8]> + AsRef<[u8]>,
        const PORT_COUNT: usize,
        const MAX_CONNECTION_COUNT: usize,
    > Manager<SingleDispatcher<S>, B, PORT_COUNT, MAX_CONNECTION_COUNT>
{
    /// Create a new service manager for the given service and port.
    ///
    /// The manager will receive data into the buffer `B`, so this buffer needs
    /// to be at least as large as the largest message this service can handle.
    ///
    /// # Examples
    ///
    /// ```
    /// struct MyService;
    ///
    /// impl Service for MyService {
    ///     type Connection = ();
    ///     type Message = ();
    ///
    ///     fn on_connect(
    ///         &self,
    ///         _port: &PortCfg,
    ///         _handle: &Handle,
    ///         _peer: &Uuid,
    ///     ) -> Result<ConnectResult<Self::Connection>> {
    ///         Ok(ConnectResult::Accept(()))
    ///     }
    ///
    ///     fn on_message(
    ///         &self,
    ///         _connection: &Self::Connection,
    ///         _handle: &Handle,
    ///         _msg: Self::Message,
    ///     ) -> Result<MessageResult> {
    ///         Ok(MessageResult::MaintainConnection)
    ///     }
    /// }
    ///
    /// let service = MyService;
    /// let cfg = PortCfg::new("com.android.trusty.rust_port_test");
    /// let buffer = [0u8; 4096];
    /// let mut manager = Manager::<_, _, 1, 1>::new(service, &[cfg], buffer);
    ///
    /// manager.run_event_loop()
    ///     .expect("Service manager encountered an error");
    /// ```
    pub fn new(service: S, port_cfg: PortCfg, buffer: B) -> Result<Self> {
        let dispatcher = SingleDispatcher::new(service, port_cfg);
        Self::new_with_dispatcher(dispatcher, buffer)
    }
}

impl<S: UnbufferedService, const PORT_COUNT: usize, const MAX_CONNECTION_COUNT: usize>
    Manager<SingleUnbufferedDispatcher<S>, [u8; 0], PORT_COUNT, MAX_CONNECTION_COUNT>
{
    /// Create a new unbuffered service manager for the given service and port.
    ///
    /// The newly created manager will not have an internal buffer.
    /// The service is responsible for reading messages explicitly from the handler.
    pub fn new_unbuffered(service: S, port_cfg: PortCfg) -> Result<Self> {
        let dispatcher = SingleUnbufferedDispatcher::new(service, port_cfg);
        Self::new_with_dispatcher(dispatcher, [])
    }
}

impl<
        D: Dispatcher,
        B: AsMut<[u8]> + AsRef<[u8]>,
        const PORT_COUNT: usize,
        const MAX_CONNECTION_COUNT: usize,
    > Manager<D, B, PORT_COUNT, MAX_CONNECTION_COUNT>
{
    /// Create a manager that can handle multiple services and ports
    ///
    /// A dispatcher handles mapping connections to services and parsing
    /// messages for the relevant service depending on which port the connection
    /// was made to. This allows multiple distinct services, each with their own
    /// message format and port to share the same event loop in the manager.
    ///
    /// See [`service_dispatcher!`] for details on how to create a dispatcher
    /// for use with this API.
    ///
    /// [`service_dispatcher!`]: crate::service_dispatcher
    ///
    /// # Examples
    /// ```
    /// service_dispatcher! {
    ///     enum ServiceDispatcher {
    ///         Service1,
    ///         Service2,
    ///     }
    /// }
    ///
    /// // Create a new dispatcher that handles two ports
    /// let dispatcher = ServiceDispatcher::<2>::new()
    ///     .expect("Could not allocate service dispatcher");
    ///
    /// let cfg = PortCfg::new(&"com.android.trusty.test_port1).unwrap();
    /// dispatcher.add_service(Rc::new(Service1), cfg).expect("Could not add service 1");
    ///
    /// let cfg = PortCfg::new(&"com.android.trusty.test_port2).unwrap();
    /// dispatcher.add_service(Rc::new(Service2), cfg).expect("Could not add service 2");
    ///
    /// Manager::<_, _, 2, 4>::new_with_dispatcher(dispatcher, [0u8; 4096])
    ///     .expect("Could not create service manager")
    ///     .run_event_loop()
    ///     .expect("Service manager exited unexpectedly");
    /// ```
    pub fn new_with_dispatcher(dispatcher: D, buffer: B) -> Result<Self> {
        if buffer.as_ref().len() < dispatcher.max_message_length() {
            return Err(TipcError::NotEnoughBuffer);
        }

        let ports: Vec<Rc<Channel<D>>> = dispatcher
            .port_configurations()
            .iter()
            .map(Channel::try_new_port)
            .collect::<Result<_>>()?;
        let ports: [Rc<Channel<D>>; PORT_COUNT] = ports
            .try_into()
            .expect("This is impossible. Array size must match expected PORT_COUNT");
        let handle_set = HandleSet::try_new(ports)?;

        Ok(Self { dispatcher, handle_set, buffer })
    }

    /// Run the service event loop.
    ///
    /// Only returns if an error occurs.
    pub fn run_event_loop(mut self) -> Result<()> {
        use trusty_sys::Error;

        loop {
            // Process the next incoming event, extracting any returned error for further
            // checking.
            let err = match self.event_loop_inner() {
                Ok(()) => continue,
                Err(err) => err,
            };

            // Check if the error is recoverable or not. If the error is not one of a
            // limited set of recoverable errors, we break from the event loop.
            match err {
                // Recoverable errors that are always ignored.
                | TipcError::SystemError(Error::TimedOut)
                | TipcError::SystemError(Error::ChannelClosed)

                // returned when peer UUID connection is not allowed.
                | TipcError::SystemError(Error::NotAllowed)

                // These are always caused by the client and so shouldn't be treated as an
                // internal error or cause the event loop to exit.
                | TipcError::ChannelClosed
                => {
                    debug!("Recoverable error ignored: {:?}", err)
                }

                // These are legitimate errors and we should be handling them, but they would be
                // better handled lower in the event loop closer to where they originate. If
                // they get propagated up here then we can't meaningfully handle them anymore,
                // so just log them and continue the loop.
                | TipcError::IncompleteWrite { .. }
                | TipcError::NotEnoughBuffer
                | TipcError::Busy
                => {
                    warn!(
                        "Received error {:?} in main event loop. This should have been handled closer to where it originated",
                        err,
                    )
                }

                _ => {
                    error!("Error occurred while handling incoming event: {:?}", err);
                    return Err(err);
                }
            }
        }
    }

    fn event_loop_inner(&mut self) -> Result<()> {
        let event = self.handle_set.wait(None)?;
        // SAFETY: This cookie was previously initialized by the handle set.
        // Its lifetime is managed by the handle set, so we are sure that
        // the cookie is still valid if the channel is still in this handle
        // set.
        let channel: Rc<Channel<D>> = unsafe { Channel::from_opaque_ptr(event.cookie) }
            .ok_or_else(|| {
                // The opaque pointer associated with this handle could not
                // be converted back into a `Channel`. This should never
                // happen, but throw an internal error if it does.
                error!("Connection cookie was invalid");
                TipcError::InvalidData
            })?;
        self.handler(channel, &event)
    }

    fn handler(&mut self, channel: Rc<Channel<D>>, event: &trusty_sys::uevent) -> Result<()> {
        // TODO: Abort on port errors?
        match &channel.ty {
            ChannelTy::Port(cfg) if event.event & (sys::IPC_HANDLE_POLL_READY as u32) != 0 => {
                self.handle_connect(&channel.handle, cfg)
            }
            ChannelTy::Connection(data) if event.event & (sys::IPC_HANDLE_POLL_MSG as u32) != 0 => {
                match self.handle_message(&channel.handle, &data) {
                    Ok(MessageResult::MaintainConnection) => Ok(()),
                    Ok(MessageResult::CloseConnection) => {
                        self.handle_set.close(channel);
                        Ok(())
                    }
                    Err(e) => {
                        error!("Could not handle message, closing connection: {:?}", e);
                        self.handle_set.close(channel);
                        Ok(())
                    }
                }
            }
            ChannelTy::Connection(data) if event.event & (sys::IPC_HANDLE_POLL_HUP as u32) != 0 => {
                self.handle_disconnect(&channel.handle, &data);
                self.handle_set.close(channel);
                Ok(())
            }

            // `SEND_UNBLOCKED` means that some previous attempt to send a message was
            // blocked and has now become unblocked. This should normally be handled by
            // the code trying to send the message, but if the sending code doesn't do so
            // then we can end up getting it here.
            _ if event.event & (sys::IPC_HANDLE_POLL_SEND_UNBLOCKED as u32) != 0 => {
                warn!("Received `SEND_UNBLOCKED` event received in main event loop. This likely means that a sent message was lost somewhere");
                Ok(())
            }

            // `NONE` is not an event we should get in practice, but if it does then it
            // shouldn't trigger an error.
            _ if event.event == (sys::IPC_HANDLE_POLL_NONE as u32) => Ok(()),

            // Treat any unrecognized events as errors by default.
            _ => {
                error!("Could not handle event {}", event.event);
                Err(TipcError::UnknownError)
            }
        }
    }

    fn handle_connect(&mut self, handle: &Handle, cfg: &PortCfg) -> Result<()> {
        let mut peer = MaybeUninit::<peer_id::uuid>::zeroed();
        // SAFETY: syscall. The port owns its handle, so it is still valid as
        // a raw fd. The peer structure outlives this call and is mutably
        // borrowed by the call to initialize the structure's data.
        let rc = unsafe { trusty_sys::accept(handle.as_raw_fd(), peer.as_mut_ptr()) as i32 };
        let connection_handle = Handle::from_raw(rc)?;
        // SAFETY: accept did not return an error, so it has successfully
        // initialized the peer structure.
        let peer = unsafe { peer.assume_init() }.into();

        // Check against access control list if we were given one
        if let Some(uuids) = cfg.uuid_allow_list {
            if !uuids.contains(&peer) {
                error!("UUID {peer:?} isn't supported.\n");
                return Err(TipcError::SystemError(trusty_sys::Error::NotAllowed));
            }
        }

        let connection_data = self.dispatcher.on_connect(&cfg, &connection_handle, &peer)?;
        if let ConnectResult::Accept(data) = connection_data {
            let connection_channel = Channel::try_new_connection(connection_handle, data)?;
            self.handle_set.add_connection(connection_channel)?;
        }

        Ok(())
    }

    fn handle_message(&mut self, handle: &Handle, data: &D::Connection) -> Result<MessageResult> {
        self.dispatcher.on_message(data, handle, self.buffer.as_mut())
    }

    fn handle_disconnect(&mut self, _handle: &Handle, data: &D::Connection) {
        self.dispatcher.on_disconnect(data);
    }
}

#[cfg(test)]
mod test {
    use super::{PortCfg, Service};
    use crate::handle::test::{first_free_handle_index, MAX_USER_HANDLES};
    use crate::{
        ConnectResult, Deserialize, Handle, Manager, MessageResult, Result, TipcError,
        UnbufferedService, Uuid,
    };
    use test::{expect, expect_eq};
    use trusty_std::alloc::FallibleVec;
    use trusty_std::ffi::{CString, FallibleCString};
    use trusty_std::format;
    use trusty_std::rc::Rc;
    use trusty_std::vec::Vec;
    use trusty_sys::Error;

    /// Maximum length of port path name
    const MAX_PORT_PATH_LEN: usize = 64;

    /// Maximum number of buffers per port
    const MAX_PORT_BUF_NUM: u32 = 64;

    /// Maximum size of port buffer
    const MAX_PORT_BUF_SIZE: u32 = 4096;

    const SRV_PATH_BASE: &str = "com.android.ipc-unittest";

    impl Service for () {
        type Connection = ();
        type Message = ();

        fn on_connect(
            &self,
            _port: &PortCfg,
            _handle: &Handle,
            _peer: &Uuid,
        ) -> Result<ConnectResult<Self::Connection>> {
            Ok(ConnectResult::Accept(()))
        }

        fn on_message(
            &self,
            _connection: &Self::Connection,
            _handle: &Handle,
            _msg: Self::Message,
        ) -> Result<MessageResult> {
            Ok(MessageResult::MaintainConnection)
        }
    }

    type Channel = super::Channel<super::SingleDispatcher<()>>;

    #[test]
    fn port_create_negative() {
        let path = [0u8; 0];

        expect_eq!(
            Channel::try_new_port(&PortCfg::new_raw(CString::try_new(&path[..]).unwrap())).err(),
            Some(TipcError::SystemError(Error::InvalidArgs)),
            "empty server path",
        );

        let mut path = format!("{}.port", SRV_PATH_BASE);

        let cfg = PortCfg::new(&path).unwrap().msg_queue_len(0);
        expect_eq!(
            Channel::try_new_port(&cfg).err(),
            Some(TipcError::SystemError(Error::InvalidArgs)),
            "no buffers",
        );

        let cfg = PortCfg::new(&path).unwrap().msg_max_size(0);
        expect_eq!(
            Channel::try_new_port(&cfg).err(),
            Some(TipcError::SystemError(Error::InvalidArgs)),
            "zero buffer size",
        );

        let cfg = PortCfg::new(&path).unwrap().msg_queue_len(MAX_PORT_BUF_NUM * 100);
        expect_eq!(
            Channel::try_new_port(&cfg).err(),
            Some(TipcError::SystemError(Error::InvalidArgs)),
            "large number of buffers",
        );

        let cfg = PortCfg::new(&path).unwrap().msg_max_size(MAX_PORT_BUF_SIZE * 100);
        expect_eq!(
            Channel::try_new_port(&cfg).err(),
            Some(TipcError::SystemError(Error::InvalidArgs)),
            "large buffers size",
        );

        while path.len() < MAX_PORT_PATH_LEN + 16 {
            path.push('a');
        }

        let cfg = PortCfg::new(&path).unwrap();
        expect_eq!(
            Channel::try_new_port(&cfg).err(),
            Some(TipcError::SystemError(Error::InvalidArgs)),
            "path is too long",
        );
    }

    #[test]
    fn port_create() {
        let mut channels: Vec<Rc<Channel>> = Vec::new();

        for i in first_free_handle_index()..MAX_USER_HANDLES - 1 {
            let path = format!("{}.port.{}{}", SRV_PATH_BASE, "test", i);
            let cfg = PortCfg::new(path).unwrap();
            let channel = Channel::try_new_port(&cfg);
            expect!(channel.is_ok(), "create ports");
            channels.try_push(channel.unwrap()).unwrap();

            expect_eq!(
                Channel::try_new_port(&cfg).err(),
                Some(TipcError::SystemError(Error::AlreadyExists)),
                "collide with existing port",
            );
        }

        // Creating one more port should succeed
        let path = format!("{}.port.{}{}", SRV_PATH_BASE, "test", MAX_USER_HANDLES - 1);
        let cfg = PortCfg::new(path).unwrap();
        let channel = Channel::try_new_port(&cfg);
        expect!(channel.is_ok(), "create ports");
        channels.try_push(channel.unwrap()).unwrap();

        // but creating colliding port should fail with different error code
        // because we actually exceeded max number of handles instead of
        // colliding with an existing path
        expect_eq!(
            Channel::try_new_port(&cfg).err(),
            Some(TipcError::SystemError(Error::NoResources)),
            "collide with existing port",
        );

        let path = format!("{}.port.{}{}", SRV_PATH_BASE, "test", MAX_USER_HANDLES);
        let cfg = PortCfg::new(path).unwrap();
        expect_eq!(
            Channel::try_new_port(&cfg).err(),
            Some(TipcError::SystemError(Error::NoResources)),
            "max number of ports reached",
        );
    }

    #[test]
    fn wait_on_port() {
        let mut channels: Vec<Rc<Channel>> = Vec::new();

        for i in first_free_handle_index()..MAX_USER_HANDLES {
            let path = format!("{}.port.{}{}", SRV_PATH_BASE, "test", i);
            let cfg = PortCfg::new(path).unwrap();
            let channel = Channel::try_new_port(&cfg);
            expect!(channel.is_ok(), "create ports");
            channels.try_push(channel.unwrap()).unwrap();
        }

        for chan in &channels {
            expect_eq!(
                chan.handle.wait(Some(0)).err(),
                Some(TipcError::SystemError(Error::TimedOut)),
                "zero timeout",
            );

            expect_eq!(
                chan.handle.wait(Some(100)).err(),
                Some(TipcError::SystemError(Error::TimedOut)),
                "non-zero timeout",
            );
        }
    }

    impl Deserialize for i32 {
        type Error = TipcError;

        const MAX_SERIALIZED_SIZE: usize = 4;

        fn deserialize(
            bytes: &[u8],
            _handles: &mut [Option<Handle>],
        ) -> core::result::Result<Self, Self::Error> {
            Ok(i32::from_ne_bytes(bytes[0..4].try_into().map_err(|_| TipcError::OutOfBounds)?))
        }
    }

    struct Service1;

    impl Service for Service1 {
        type Connection = ();
        type Message = ();

        fn on_connect(
            &self,
            _port: &PortCfg,
            _handle: &Handle,
            _peer: &Uuid,
        ) -> Result<ConnectResult<Self::Connection>> {
            Ok(ConnectResult::Accept(()))
        }

        fn on_message(
            &self,
            _connection: &Self::Connection,
            handle: &Handle,
            _msg: Self::Message,
        ) -> Result<MessageResult> {
            handle.send(&1i32)?;
            Ok(MessageResult::MaintainConnection)
        }
    }

    struct Service2;

    impl Service for Service2 {
        type Connection = ();
        type Message = ();

        fn on_connect(
            &self,
            _port: &PortCfg,
            _handle: &Handle,
            _peer: &Uuid,
        ) -> Result<ConnectResult<Self::Connection>> {
            Ok(ConnectResult::Accept(()))
        }

        fn on_message(
            &self,
            _connection: &Self::Connection,
            handle: &Handle,
            _msg: Self::Message,
        ) -> Result<MessageResult> {
            handle.send(&2i32)?;
            Ok(MessageResult::MaintainConnection)
        }
    }

    struct Service3;

    impl UnbufferedService for Service3 {
        type Connection = ();

        fn on_connect(
            &self,
            _port: &PortCfg,
            _handle: &Handle,
            _peer: &Uuid,
        ) -> Result<ConnectResult<Self::Connection>> {
            Ok(ConnectResult::Accept(()))
        }

        fn on_message(
            &self,
            _connection: &Self::Connection,
            handle: &Handle,
            _buffer: &mut [u8],
        ) -> Result<MessageResult> {
            handle.send(&3i32)?;
            Ok(MessageResult::MaintainConnection)
        }
    }

    wrap_service!(WrappedService2(Service2: Service));
    wrap_service!(WrappedService3(Service3: UnbufferedService));

    service_dispatcher! {
        enum TestServiceDispatcher {
            Service1,
            Service2,
            Service3,
            WrappedService2,
            WrappedService3,
        }
    }

    #[test]
    fn multiple_services() {
        let mut dispatcher = TestServiceDispatcher::<5>::new().unwrap();

        let path1 = format!("{}.port.{}", SRV_PATH_BASE, "testService1");
        let cfg = PortCfg::new(&path1).unwrap();
        dispatcher.add_service(Rc::new(Service1), cfg).expect("Could not add service 1");

        let path2 = format!("{}.port.{}", SRV_PATH_BASE, "testService2");
        let cfg = PortCfg::new(&path2).unwrap();
        dispatcher.add_service(Rc::new(Service2), cfg).expect("Could not add service 2");

        let path = format!("{}.port.{}", SRV_PATH_BASE, "testService3");
        let cfg = PortCfg::new(&path).unwrap();
        dispatcher.add_service(Rc::new(Service3), cfg).expect("Could not add service 3");

        let path = format!("{}.port.{}", SRV_PATH_BASE, "testWrappedService2");
        let cfg = PortCfg::new(&path).unwrap();
        dispatcher
            .add_service(Rc::new(WrappedService2(Service2)), cfg)
            .expect("Could not add wrapped service 2");

        let path = format!("{}.port.{}", SRV_PATH_BASE, "testWrappedService3");
        let cfg = PortCfg::new(&path).unwrap();
        dispatcher
            .add_service(Rc::new(WrappedService3(Service3)), cfg)
            .expect("Could not add wrapped service 3");

        let buffer = [0u8; 4096];
        Manager::<_, _, 5, 4>::new_with_dispatcher(dispatcher, buffer)
            .expect("Could not create service manager");
    }

    #[test]
    fn unbuffered_service() {
        let path = format!("{}.port.{}", SRV_PATH_BASE, "unbufferedService");
        let cfg = PortCfg::new(&path).unwrap();
        Manager::<_, _, 1, 4>::new_unbuffered(Service3, cfg)
            .expect("Could not create service manager");
    }
}

#[cfg(test)]
mod multiservice_with_lifetimes_tests {
    use super::*;
    use core::marker::PhantomData;
    #[allow(unused)]
    use trusty_std::alloc::FallibleVec;

    const SRV_PATH_BASE: &str = "com.android.ipc-unittest-lifetimes";

    pub(crate) struct Service1<'a> {
        phantom: PhantomData<&'a u32>,
    }

    impl<'a> Service for Service1<'a> {
        type Connection = ();
        type Message = ();

        fn on_connect(
            &self,
            _port: &PortCfg,
            _handle: &Handle,
            _peer: &Uuid,
        ) -> Result<ConnectResult<Self::Connection>> {
            Ok(ConnectResult::Accept(()))
        }

        fn on_message(
            &self,
            _connection: &Self::Connection,
            handle: &Handle,
            _msg: Self::Message,
        ) -> Result<MessageResult> {
            handle.send(&2i32)?;
            Ok(MessageResult::MaintainConnection)
        }
    }

    pub(crate) struct Service2<'a> {
        phantom: PhantomData<&'a u32>,
    }

    impl<'a> Service for Service2<'a> {
        type Connection = ();
        type Message = ();

        fn on_connect(
            &self,
            _port: &PortCfg,
            _handle: &Handle,
            _peer: &Uuid,
        ) -> Result<ConnectResult<Self::Connection>> {
            Ok(ConnectResult::Accept(()))
        }

        fn on_message(
            &self,
            _connection: &Self::Connection,
            handle: &Handle,
            _msg: Self::Message,
        ) -> Result<MessageResult> {
            handle.send(&2i32)?;
            Ok(MessageResult::MaintainConnection)
        }
    }

    service_dispatcher! {
        pub(crate) enum TestServiceLifetimeDispatcher<'a> {
            Service1<'a>,
            Service2<'a>,
        }
    }

    #[test]
    fn manager_creation() {
        let mut dispatcher = TestServiceLifetimeDispatcher::<2>::new().unwrap();

        let path1 = format!("{}.port.{}", SRV_PATH_BASE, "testService1");
        let cfg = PortCfg::new(&path1).unwrap();
        dispatcher
            .add_service(Rc::new(Service1 { phantom: PhantomData }), cfg)
            .expect("Could not add service 1");

        let path2 = format!("{}.port.{}", SRV_PATH_BASE, "testService2");
        let cfg = PortCfg::new(&path2).unwrap();
        dispatcher
            .add_service(Rc::new(Service2 { phantom: PhantomData }), cfg)
            .expect("Could not add service 2");

        let buffer = [0u8; 4096];
        Manager::<_, _, 2, 4>::new_with_dispatcher(dispatcher, buffer)
            .expect("Could not create service manager");
    }
}

#[cfg(test)]
mod uuid_tests {
    use super::Uuid;
    use test::{expect, expect_eq};

    #[test]
    fn uuid_parsing() {
        let uuid = Uuid::new(0, 0, 0, [0; 8]);
        let uuid_string = "00000000-0000-0000-0000-000000000000".to_string();
        expect_eq!(uuid.to_string(), uuid_string);
        let uuid_from_str = Uuid::new_from_string(&uuid_string);
        expect!(uuid_from_str.is_ok(), "couldn't parse uuid string");
        let uuid_from_str = uuid_from_str.unwrap();
        expect_eq!(uuid, uuid_from_str, "uuid should match");
        let uuid2 = Uuid::new(1262561249, 51804, 17255, [189, 181, 5, 22, 64, 5, 183, 196]);
        let uuid_string_2 = "4b4127e1-ca5c-4367-bdb5-05164005b7c4".to_string();
        let uuid2_from_str = Uuid::new_from_string(&uuid_string_2);
        expect!(uuid2_from_str.is_ok(), "couldn't parse uuid string");
        let uuid2_from_str = uuid2_from_str.unwrap();
        expect_eq!(uuid2, uuid2_from_str, "uuid should match");
        let bad_uuid_from_str = Uuid::new_from_string("4b4127e1-ca5c-4367-bdb5-05164005b7c45");
        expect!(bad_uuid_from_str.is_err(), "shouldn't be able to parse string");
        let bad_uuid_from_str = Uuid::new_from_string("4b4127e1-ca5c-4367-bdbq-05164005b7c4");
        expect!(bad_uuid_from_str.is_err(), "shouldn't be able to parse string");
        let bad_uuid_from_str = Uuid::new_from_string("4b4127e1-ca5c-4367-bdb5005164005b7c4");
        expect!(bad_uuid_from_str.is_err(), "shouldn't be able to parse string");
        let bad_uuid_from_str = Uuid::new_from_string("4b41-7e1-ca5c-4367-bdb5-05164005b7c4");
        expect!(bad_uuid_from_str.is_err(), "shouldn't be able to parse string");
        let bad_uuid_from_str = Uuid::new_from_string("4b4127e1-ca5c-4367-bd-b505164005b7c4");
        expect!(bad_uuid_from_str.is_err(), "shouldn't be able to parse string");
    }
}

#[cfg(test)]
mod client_id_tests {
    use super::{ClientIdentifier, Uuid, VmIdentifier};
    use test::{assert_eq, assert_ok};

    #[test]
    fn client_id_round_trip() {
        let uuid_string = "4b4127e1-ca5c-4367-bdb5-05164005b7c4".to_string();
        let uuid = Uuid::new_from_string(&uuid_string).unwrap();
        let uuid_client_id = ClientIdentifier::UUID(uuid.clone());
        let client_seq_num_client_id = ClientIdentifier::ClientSeqNumber(0x0123_4567_89ab_cedf);
        let vm_id_client_id = ClientIdentifier::VMID(VmIdentifier::Ffa(0x1234));

        // Simulate the steps taken place when ClientIdentifier is used with
        // rpc-binder C++ interface
        for cid in [uuid_client_id, client_seq_num_client_id, vm_id_client_id] {
            let c_repr = cid.c_repr();
            let reconstructed = assert_ok!(ClientIdentifier::try_from_c_repr(c_repr.as_generic()));

            assert_eq!(cid, reconstructed);
        }
    }
}
