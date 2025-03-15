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
///! A handle object type (i.e. a wrapper type for the `Handle`) that can be stored in the
///! `RawHandleSet`. This is similar to `Channel`, except that this has a field to hold a reference
///! to the service that corresponds to the `Handle`.
use crate::{raw::HandleSetWrapper, raw::Handler, Handle, PortCfg, UnbufferedService};
use alloc::sync::Arc;
use core::fmt;
use log::warn;

/// A type of a handle object that can be stored in the `RawHandleSet`.
pub struct ServiceHandle<S: UnbufferedService> {
    pub(crate) handle: Handle,
    pub(crate) service: Arc<S>,
    pub(crate) ty: HandleType<S::Connection>,
}

pub struct PortWrapper<'a, S: UnbufferedService> {
    pub(crate) handle_set_wrapper: &'a HandleSetWrapper<S>,
    pub(crate) service_handle: Arc<ServiceHandle<S>>,
}

impl<'a, S: UnbufferedService> PortWrapper<'a, S> {
    pub fn new(
        handle_set_wrapper: &'a HandleSetWrapper<S>,
        service_handle: Arc<ServiceHandle<S>>,
    ) -> PortWrapper<'a, S> {
        Self { handle_set_wrapper, service_handle }
    }
}

impl<S: UnbufferedService> Drop for PortWrapper<'_, S> {
    fn drop(&mut self) {
        if let Err(error) = self.handle_set_wrapper.remove(self.service_handle.clone()) {
            warn!("Failed to remove handle from the handle set: {error:?}");
        };
        let ref_count = Arc::strong_count(&self.service_handle);
        if ref_count > 1 {
            warn!("PortWrapper leaking handle {}", self.service_handle.get_raw_fd_id());
        }
    }
}

impl<S: UnbufferedService> ServiceHandle<S> {
    /// Create a port type handle object
    pub fn new_port_wrapper(cfg: PortCfg, handle: Handle, service: Arc<S>) -> Self {
        Self { handle, service, ty: HandleType::Port(cfg) }
    }
    /// Create a connection type handle object
    pub fn new_connection_wrapper(
        handle: Handle,
        service: Arc<S>,
        connection: S::Connection,
    ) -> Self {
        Self { handle, service, ty: HandleType::Connection(connection) }
    }
}

impl<S: UnbufferedService> Handler for ServiceHandle<S> {
    fn get_raw_fd_id(&self) -> i32 {
        self.handle.as_raw_fd()
    }
}

/// Enum representing the type of the handle in the `ServiceHandle`.
pub enum HandleType<C> {
    /// Service port with a configuration describing the port
    Port(PortCfg),

    /// Client connection
    Connection(C),
}

impl<C> fmt::Debug for HandleType<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HandleType::Port(cfg) => write!(f, "HandleType::Port({:?})", cfg),
            HandleType::Connection(_) => write!(f, "HandleType::Connection"),
        }
    }
}
