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

mod vintf_services;

pub use vintf_services::{get_supported_vintf_services, VintfService};

use alloc::ffi::CString;
use binder::{ExceptionCode, FromIBinder, ParcelFileDescriptor, StatusCode, Strong};
use rpcbinder::{FileDescriptorTransportMode, RpcSession};
use std::ffi::CStr;
use std::os::fd::IntoRawFd;
use trusty_binder_accessor::aidl::trusty::os::ITrustyAccessor::{
    ITrustyAccessor, ERROR_CONNECTION_INFO_NOT_FOUND, ERROR_FAILED_TO_CONNECT_EACCES,
};

// At the time of writing, this max is enforced by the trusty kernel
// which defines MAX_PORT_PATH_LEN
const MAX_TIPC_PORT_NAME_LEN: usize = 64;
const TRUSTY_BINDER_RPC_PORT_SUFFIX: &str = ".bnd";
const TRUSTED_HAL_COMMON_PREFIX: &str = "android.hardware.security.see";
const TRUSTED_HAL_REPLACEMENT_PREFIX: &str = "ahss";
pub const HANDOVER_SERVICE_PREFIX: &str = "IHandover/";
const HANDOVER_SERVICE_SUFFIX: &str = ".hnd";

#[derive(Debug, PartialEq)]
enum Error {
    /// A connection to the given port could not be established.
    ConnectionFailed,
    /// A connection was established to the given port, but setting up a binder client
    /// failed.
    BinderSetupFailed(StatusCode),
}

/// Retrieve a service by name. This function will block until a service is available and
/// may wait indefinitely if it never becomes available.
///
/// In trusty, services are exposed to other apps by their port names.
/// The convention in Android Binder is to use {binder_descriptor}/{service_instance}
/// as the pattern for service names. When possible, trusty services that are defined in AIDL
/// should adhere to this convention.
/// E.g. "android.hardware.security.see.storage.ISecureStorage/tee"
///
/// Note that trusty port names cannot be more than 64 characters, so `name` must be
/// adhere to that constraint. To prevent collisions between non-binder trusty services,
/// we add a suffix of `.bnd` to all service names in this function.
///
/// TODO: b/395096422 - Make this more performant once we have a CPP lib or a rust API
/// that can access SpIBinder directly.
pub fn wait_for_interface<T: FromIBinder + ?Sized>(name: &str) -> Result<Strong<T>, StatusCode> {
    let c_port_name = service_name_to_trusty_c_port(name)?;
    let port_name = c_port_name.as_c_str().to_str().map_err(|_| StatusCode::BAD_VALUE)?;

    // First try and see if our service is gated by an ITrustyAccessor implementation.
    // This is a binder service exposed on the port we've connected to that acts as
    // an intermediary for resolving connections to the requested service. It was originally
    // introduced to allow for authentication and authorization of the caller before handing
    // back a handle to the requested service.
    let mut session = get_new_rpc_session();
    match setup_trusty_client(&session, &c_port_name) {
        Ok(accessor) => {
            let service_fd = fd_from_accessor(&accessor, name)?;

            return rpcbinder_from_parcel_fd(service_fd).inspect_err(|&status_code| {
                log::error!(
                    "Failed to setup binder on fd returned from ITrustyAccessor for {:?} {}",
                    &port_name,
                    status_code
                );
            });
        }
        Err(Error::ConnectionFailed) => {
            // If the port we requested didn't exist this would have blocked forever. So we assume
            // that the service listening on this port rejected our connection.
            return Err(StatusCode::PERMISSION_DENIED);
        }
        Err(Error::BinderSetupFailed(s)) => {
            log::debug!(
                "failed to setup client to ITrustyAccessor for {:?} {:?}. Will try a direct \
            connection.",
                &port_name,
                s
            );
        }
    }

    // The binder on the other end was likely not an accessor, try a direct connection.
    session = get_new_rpc_session();
    setup_trusty_client(&session, &c_port_name).map_err(|e| {
        log::error!("failed to setup binder on {:?} {:?}", &port_name, e);
        match e {
            // This is an unexpected case. We've already successfully connected to this port once.
            Error::ConnectionFailed => StatusCode::PERMISSION_DENIED,
            Error::BinderSetupFailed(s) => s,
        }
    })
}

fn fd_from_accessor(
    accessor: &Strong<dyn ITrustyAccessor>,
    name: &str,
) -> Result<ParcelFileDescriptor, StatusCode> {
    // Before we attempt to add a connection, ensure that we've obtained an Accessor
    // for the correct service name.
    let accessor_instance_name = accessor.getInstanceName().map_err(|status_code| {
        log::error!(
            "Failed to resolve ITrustyAccessor instance name at {:?} {}",
            name,
            status_code
        );

        StatusCode::NAME_NOT_FOUND
    })?;

    if accessor_instance_name != name {
        log::error!(
            "Provided service name {:?} does not match the instance name resolved from \
        ITrustyAccessor {}",
            name,
            accessor_instance_name
        );

        return Err(StatusCode::NAME_NOT_FOUND);
    }

    let service_fd = accessor.addConnection().map_err(|status| {
        log::error!("Failed to retrieve FD from accessor for {:?} {}", name, status);

        match status.exception_code() {
            ExceptionCode::SERVICE_SPECIFIC => match status.service_specific_error() {
                ERROR_FAILED_TO_CONNECT_EACCES => StatusCode::PERMISSION_DENIED,
                ERROR_CONNECTION_INFO_NOT_FOUND => StatusCode::NAME_NOT_FOUND,
                _ => StatusCode::UNKNOWN_ERROR,
            },
            ExceptionCode::TRANSACTION_FAILED => status.transaction_error(),
            _ => StatusCode::NAME_NOT_FOUND,
        }
    })?;

    Ok(service_fd)
}

fn rpcbinder_from_parcel_fd<T: FromIBinder + ?Sized>(
    parceled_fd: ParcelFileDescriptor,
) -> Result<Strong<T>, StatusCode> {
    let session = get_new_rpc_session();
    let raw_fd = parceled_fd.into_raw_fd();

    session.setup_preconnected_client(move || Some(raw_fd))
}

/// Pretty much a re-implementation of RpcSession::setup_trusty_client but does not panic
/// when a connection fails. As a bonus, we can differentiate between a failed raw connection
/// and other binder-specific errors.
fn setup_trusty_client<T: FromIBinder + ?Sized>(
    session: &RpcSession,
    port: &CStr,
) -> Result<Strong<T>, Error> {
    let h = tipc::Handle::connect(port).map_err(|e| {
        log::error!("Failed to connect to port {:?} {:?}", port, e);
        Error::ConnectionFailed
    })?;
    // Do not close the handle at the end of the scope
    let fd = h.as_raw_fd();
    core::mem::forget(h);
    session.setup_preconnected_client(|| Some(fd)).map_err(|status_code| {
        log::debug!("failed to setup preconnected client on port {:?} {}", port, status_code);

        Error::BinderSetupFailed(status_code)
    })
}

fn get_new_rpc_session() -> RpcSession {
    let session = RpcSession::new();
    session.set_file_descriptor_transport_mode(FileDescriptorTransportMode::Trusty);

    session
}

// Many trusted hal interfaces (VINTF stable interfaces to the TEE) are too long
// for the current max port name (64 characters including a null byte). We work
// around this temporarily by taking advantage of a common prefix they all have
// and map "android.hardware.security.see" to "ahss".
//
// So for example, "android.hardware.security.see.authmgr.IAuthMgrAuthorization/default"
// will result in a port name of ahss.authmgr.IAuthMgrAuhtorization/default .
//
// This function also constructs (shortened) port names for the handover services associated with
// the trusted services. An example handover service's name given as:
// "IHandover/android.hardware.security.see.secure.storage.ISecureStorage/default" will result in a
// port name of: "ahss.secure.storage.ISecureStorage/default.hnd".
fn try_long_service_name_to_port(service_name: &str) -> Result<String, StatusCode> {
    let mut port_name = String::new();

    match service_name.split_once(TRUSTED_HAL_COMMON_PREFIX) {
        // Since we're in this function, we already know the given service_name is too long
        // and a None here means we won't shorten it since our prefix wasn't found.
        None => Err(port_size_err(service_name)),
        // In this case our pattern was found, but not at the front of the str. We first check
        // whether the front of the str matches the prefix for the handover service and handle it
        // accordingly.
        Some((pre, post)) if pre == HANDOVER_SERVICE_PREFIX => {
            build_shortened_port_name(&mut port_name, post, HANDOVER_SERVICE_SUFFIX)
        }
        // The front of the str doesn't match any known prefix. Therefore, it is not a valid case
        // for our name shortening. Since we're in this function we now know we have a name that's
        // too long and we can't shorten.
        Some((pre, _)) if !pre.is_empty() => Err(port_size_err(service_name)),
        Some((_, post)) => {
            build_shortened_port_name(&mut port_name, post, TRUSTY_BINDER_RPC_PORT_SUFFIX)
        }
    }
}

fn build_shortened_port_name(
    port_name: &mut String,
    service_name: &str,
    suffix: &str,
) -> Result<String, StatusCode> {
    port_name
        .try_reserve(TRUSTED_HAL_REPLACEMENT_PREFIX.len() + service_name.len() + suffix.len())
        .map_err(|_| StatusCode::NO_MEMORY)?;

    port_name.push_str(TRUSTED_HAL_REPLACEMENT_PREFIX);
    port_name.push_str(service_name);
    port_name.push_str(suffix);

    // Check size again. It's possible we weren't able to shorten the name enough.
    if !is_valid_port_len(port_name.len()) {
        return Err(port_size_err(port_name));
    }

    Ok(port_name.clone())
}

fn is_valid_port_len(port_len: usize) -> bool {
    // Note that if the name is equal to the max, we fail because
    // these ports will eventually be represented as CStrings and passed
    // to the trusty kernel to check their sizes so we need room for the
    // nul byte.
    port_len < MAX_TIPC_PORT_NAME_LEN
}

fn port_size_err(service_name: &str) -> StatusCode {
    log::error!(
        "Cannot create port name from {:?} within trusty port length limit of {}",
        service_name,
        MAX_TIPC_PORT_NAME_LEN
    );

    StatusCode::BAD_VALUE
}

/// A helper to transform a binder service name to a trusty port name.
/// A suffix (".bnd") is added to the service name to identify it as a port that is serving
/// binders.
///
/// Some known trusted hal services, those that start with android.hardware.security.see
/// are transformed to fit within trusty's port length limits.
///
/// Similarly, this function also transforms the handover services' names to a trusty port name.
/// A handover service name is detected by the prefix: "IHandover/", which is removed in the
/// returned port name. A different suffix of ".hnd" is added to the returned port name.
pub fn service_name_to_trusty_port(service_name: &str) -> Result<String, StatusCode> {
    let is_handover_service_name = service_name.starts_with(HANDOVER_SERVICE_PREFIX);
    let service_name_len = if is_handover_service_name {
        service_name.len() - HANDOVER_SERVICE_PREFIX.len() + HANDOVER_SERVICE_SUFFIX.len()
    } else {
        service_name.len() + TRUSTY_BINDER_RPC_PORT_SUFFIX.len()
    };

    // TODO: b/403531416 - remove this once longer port names are supported in trusty
    if !is_valid_port_len(service_name_len) {
        return try_long_service_name_to_port(service_name);
    }

    let mut port_name = String::new();
    port_name.try_reserve(service_name_len).map_err(|_| StatusCode::NO_MEMORY)?;

    if is_handover_service_name {
        port_name.push_str(extract_handover_service_name(service_name)?);
        port_name.push_str(HANDOVER_SERVICE_SUFFIX);
    } else {
        port_name.push_str(service_name);
        port_name.push_str(TRUSTY_BINDER_RPC_PORT_SUFFIX);
    }

    Ok(port_name)
}

/// Get a CString port name from a service_name.
/// See `service_name_to_trusty_port` for details.
pub fn service_name_to_trusty_c_port(service_name: &str) -> Result<CString, StatusCode> {
    let mut port_name = service_name_to_trusty_port(service_name)?;
    // Ensure we have room for the null byte that CString construction will add.
    port_name.try_reserve_exact(1).map_err(|_| StatusCode::NO_MEMORY)?;

    CString::new(port_name).map_err(|_| StatusCode::BAD_VALUE)
}

// A helper function to extract the handover service name by removing the prefix: "IHandover".
fn extract_handover_service_name(prefixed_handover_service_name: &str) -> Result<&str, StatusCode> {
    match prefixed_handover_service_name.split_once(HANDOVER_SERVICE_PREFIX) {
        // In this case our pattern was found, but not at the front of the str, which
        // is not a valid case.
        Some((pre, _)) if !pre.is_empty() => Err(StatusCode::BAD_VALUE),
        Some((_, post)) => Ok(post),
        // Since we're in this function, we already know the given service_name should contain the
        // prefix. So this match arm cannot be hit.
        None => Err(StatusCode::BAD_VALUE),
    }
}
