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

//! A wrapper around the `RawHandleSet`, that is aware of the handle wrapper type stored in the
//! raw handle set. The wrapper also enforces the limits on ports and connections per TA.
use crate::raw::{service_handle::PortWrapper, HandleType, RawHandleSet, ServiceHandle};
use crate::{
    ConnectResult, Handle, MessageResult, PortCfg, Result, TipcError, UnbufferedService, Uuid,
};
use alloc::sync::Arc;
use log::error;
use std::collections::vec_deque::VecDeque;
use std::sync::Mutex;
use trusty_std::TryClone;

/// Encapsulate a work item to call on_connect on a given service
pub struct ToConnect<S: UnbufferedService> {
    handle: Handle,
    service: Arc<S>,
    port: PortCfg,
    // TODO: This should be removed in the long term
    uuid: Uuid,
}

impl<S: UnbufferedService> ToConnect<S> {
    /// Constructor
    pub fn new(handle: Handle, service: Arc<S>, port: PortCfg, uuid: Uuid) -> Self {
        Self { handle, service, port, uuid }
    }

    /// Handle on_connect work item
    pub fn do_on_connect(&self) -> Result<ConnectResult<S::Connection>> {
        self.service.on_connect(&self.port, &self.handle, &self.uuid)
    }
}

/// Instructions about the work to do
pub enum WorkToDo<S: UnbufferedService> {
    Connect(ToConnect<S>),
}

/// A wrapper around the `RawHandleSet` exposed to the TAs and the event loop runner.
pub struct HandleSetWrapper<S: UnbufferedService> {
    // work queue should be emptied before waiting on the handle set
    work_queue: Mutex<VecDeque<WorkToDo<S>>>,
    handle_set: Mutex<RawHandleSet<ServiceHandle<S>>>,
}

impl<S: UnbufferedService> HandleSetWrapper<S> {
    pub fn new() -> Result<Self> {
        Ok(Self {
            work_queue: Mutex::new(VecDeque::<WorkToDo<S>>::new()),
            handle_set: Mutex::new(RawHandleSet::<ServiceHandle<S>>::new()?),
        })
    }

    fn handle_work_queue(&self) -> Result<()> {
        let mut wq = self.work_queue.lock().unwrap();
        while let Some(work_item) = wq.pop_front() {
            let WorkToDo::Connect(to_connect) = work_item;
            match to_connect.do_on_connect()? {
                ConnectResult::Accept(conn) => {
                    return self
                        .add_connection(conn, to_connect.handle, to_connect.service)
                        .map(|_| ());
                }
                ConnectResult::CloseConnection => {
                    error!("Connection closed by the service in on_connect.");
                    return Err(TipcError::ChannelClosed);
                }
            }
        }
        Ok(())
    }

    /// Waits on the raw handle set and returns the event cookie object and the event.
    pub fn wait(&self, timeout: Option<u32>) -> Result<(Arc<ServiceHandle<S>>, u32)> {
        self.handle_work_queue()?;
        let hs = self.handle_set.lock().unwrap();
        hs.wait(timeout)
    }

    /// Add a port type connection to the handle set wrapper
    pub fn add_port(&self, cfg: &PortCfg, service: Arc<S>) -> Result<PortWrapper<S>> {
        // SAFETY: syscall, config path is borrowed and outlives the call.
        // Return value is either a negative error code or a valid handle.
        let rc = unsafe {
            trusty_sys::port_create(
                cfg.get_path().as_ptr(),
                cfg.get_msg_queue_len(),
                cfg.get_msg_max_size(),
                cfg.get_flags(),
            )
        };
        if rc < 0 {
            Err(TipcError::from_uapi(rc))
        } else {
            let raw_handle_fd = rc as i32;
            let handle = Handle::from_raw(raw_handle_fd)?;
            let service_handle =
                Arc::new(ServiceHandle::<S>::new_port_wrapper(cfg.try_clone()?, handle, service));
            let hs = self.handle_set.lock().unwrap();
            hs.register(service_handle.clone())?;
            Ok(PortWrapper::new(self, service_handle))
        }
    }

    /// Dispatch an event on a port handle
    pub fn handle_connect(&self, service_handle: Arc<ServiceHandle<S>>) -> Result<()> {
        if let HandleType::Port(cfg) = &service_handle.ty {
            let mut peer = Uuid::from_bytes(&[0; Uuid::UUID_BYTE_LEN]);
            // SAFETY: syscall. The port owns its handle, so it is still valid as
            // a raw fd. The peer structure outlives this call and is mutably
            // borrowed by the call to initialize the structure's data.
            let rc = unsafe {
                trusty_sys::accept(service_handle.handle.as_raw_fd(), peer.as_mut_ptr()) as i32
            };
            let connection_handle = Handle::from_raw(rc)?;

            // Check against access control list if we were given one
            if let Some(uuids) = cfg.get_uuid_allow_list() {
                if !uuids.contains(&peer) {
                    error!("UUID {peer:?} isn't supported.\n");
                    return Err(TipcError::SystemError(trusty_sys::Error::NotAllowed));
                }
            }
            let connect_result =
                service_handle.service.on_connect(&cfg, &connection_handle, &peer)?;
            if let ConnectResult::Accept(conn) = connect_result {
                self.add_connection(conn, connection_handle, Arc::clone(&service_handle.service))
                    .map(|_| ())
            } else {
                error!("Connection closed");
                return Err(TipcError::ChannelClosed);
            }
        } else {
            error!("A port type handle is expected. Received: {:?}", &service_handle.ty);
            return Err(TipcError::InvalidData);
        }
    }

    /// Register a connection handle in the handle set
    pub fn add_connection(
        &self,
        connection: S::Connection,
        handle: Handle,
        service: Arc<S>,
    ) -> Result<()> {
        let service_handle =
            ServiceHandle::<S>::new_connection_wrapper(handle, service, connection);
        let hs = self.handle_set.lock().unwrap();
        hs.register(Arc::new(service_handle))
    }

    /// Dispatch an event on a port handle
    pub fn handle_message(&self, service_handle: Arc<ServiceHandle<S>>) -> Result<MessageResult> {
        if let HandleType::Connection(conn) = &service_handle.ty {
            service_handle.service.on_message(conn, &service_handle.handle, &mut [])
        } else {
            error!("A connection type handle is expected. Received: {:?}", &service_handle.ty);
            return Err(TipcError::InvalidData);
        }
    }

    /// Dispatch a connection close event
    pub fn handle_disconnect(&self, service_handle: Arc<ServiceHandle<S>>) -> Result<()> {
        if let HandleType::Connection(conn) = &service_handle.ty {
            service_handle.service.on_disconnect(conn)
        } else {
            error!("A connection type handle is expected. Received: {:?}", &service_handle.ty);
            return Err(TipcError::InvalidData);
        }
        self.remove(service_handle)?;
        Ok(())
    }

    /// Remove a handle from the handle set
    pub fn remove(&self, service_handle: Arc<ServiceHandle<S>>) -> Result<()> {
        let hs = self.handle_set.lock().unwrap();
        hs.remove(service_handle)
    }

    /// Remove a handle from the handle set
    pub fn remove_raw(&self, handle: i32) -> Result<Arc<ServiceHandle<S>>> {
        let hs = self.handle_set.lock().unwrap();
        hs.remove_raw(handle)
    }

    /// Add a work item
    pub fn add_work(&self, todo: WorkToDo<S>) {
        let mut wq = self.work_queue.lock().unwrap();
        wq.push_back(todo);
    }
}

#[cfg(test)]
mod test {
    use crate::handle::test::{first_free_handle_index, MAX_USER_HANDLES};
    use crate::raw::{raw_handle_set::Handler, HandleSetWrapper};
    use crate::{
        ConnectResult, Handle, MessageResult, PortCfg, Result, Service, TipcError,
        UnbufferedService, Uuid,
    };
    use alloc::sync::Arc;
    use test::{expect, expect_eq};
    use trusty_std::ffi::{CString, FallibleCString as _};
    use trusty_sys::Error;

    const SRV_PATH_BASE: &str = "com.android.ipc-raw-unittest";
    /// Maximum length of port path name
    const MAX_PORT_PATH_LEN: usize = 64;
    /// Maximum number of buffers per port
    const MAX_PORT_BUF_NUM: u32 = 64;

    /// Maximum size of port buffer
    const MAX_PORT_BUF_SIZE: u32 = 4096;

    #[test]
    fn port_create_negative() {
        let path = [0u8; 0];
        let handle_set_wrapper = HandleSetWrapper::<()>::new().unwrap();
        let service = Arc::new(());
        let cfg = PortCfg::new_raw(CString::try_new(&path[..]).unwrap());
        let err = handle_set_wrapper.add_port(&cfg, service.clone()).err();
        expect_eq!(err, Some(TipcError::SystemError(Error::InvalidArgs)), "empty server path");

        let mut path = format!("{}.port", SRV_PATH_BASE);

        let cfg = PortCfg::new(&path).unwrap().msg_queue_len(0);
        expect_eq!(
            handle_set_wrapper.add_port(&cfg, service.clone()).err(),
            Some(TipcError::SystemError(Error::InvalidArgs)),
            "no buffers",
        );

        let cfg = PortCfg::new(&path).unwrap().msg_max_size(0);
        expect_eq!(
            handle_set_wrapper.add_port(&cfg, service.clone()).err(),
            Some(TipcError::SystemError(Error::InvalidArgs)),
            "zero buffer size",
        );

        let cfg = PortCfg::new(&path).unwrap().msg_queue_len(MAX_PORT_BUF_NUM * 100);
        expect_eq!(
            handle_set_wrapper.add_port(&cfg, service.clone()).err(),
            Some(TipcError::SystemError(Error::InvalidArgs)),
            "large number of buffers",
        );

        let cfg = PortCfg::new(&path).unwrap().msg_max_size(MAX_PORT_BUF_SIZE * 100);
        expect_eq!(
            handle_set_wrapper.add_port(&cfg, service.clone()).err(),
            Some(TipcError::SystemError(Error::InvalidArgs)),
            "large buffers size",
        );

        while path.len() < MAX_PORT_PATH_LEN + 16 {
            path.push('a');
        }

        let cfg = PortCfg::new(&path).unwrap();
        expect_eq!(
            handle_set_wrapper.add_port(&cfg, service.clone()).err(),
            Some(TipcError::SystemError(Error::InvalidArgs)),
            "path is too long",
        );
    }

    #[test]
    fn port_create() {
        let handle_set_wrapper = HandleSetWrapper::<()>::new().unwrap();
        let mut ports = Vec::new();
        let service = Arc::new(());

        for i in first_free_handle_index()..MAX_USER_HANDLES - 2 {
            let path = format!("{}.port.{}{}", SRV_PATH_BASE, "test", i);
            let cfg = PortCfg::new(path).unwrap();
            let result = handle_set_wrapper.add_port(&cfg, service.clone());
            ports.push(result.unwrap());

            expect_eq!(
                handle_set_wrapper.add_port(&cfg, service.clone()).err(),
                Some(TipcError::SystemError(Error::AlreadyExists)),
                "collide with existing port"
            );
        }
        // Creating one more port should succeed
        let path = format!("{}.port.{}{}", SRV_PATH_BASE, "test", MAX_USER_HANDLES - 1);
        let cfg = PortCfg::new(path).unwrap();
        let result = handle_set_wrapper.add_port(&cfg, service.clone());
        expect!(result.is_ok(), "create one more port");

        // but creating colliding port should fail with different error code
        // because we actually exceeded max number of handles instead of
        // colliding with an existing path
        expect_eq!(
            handle_set_wrapper.add_port(&cfg, service.clone()).err(),
            Some(TipcError::SystemError(Error::NoResources)),
            "collide with existing port",
        );

        let path = format!("{}.port.{}{}", SRV_PATH_BASE, "test", MAX_USER_HANDLES);
        let cfg = PortCfg::new(path).unwrap();
        expect_eq!(
            handle_set_wrapper.add_port(&cfg, service.clone()).err(),
            Some(TipcError::SystemError(Error::NoResources)),
            "max number of ports reached",
        );
    }

    #[test]
    fn add_port() {
        let handle_set_wrapper = HandleSetWrapper::<()>::new().unwrap();
        let path = format!("{}.port.{}", SRV_PATH_BASE, "test");
        let cfg = PortCfg::new(path).unwrap();
        let service = Arc::new(());
        {
            let port_wrapper = handle_set_wrapper.add_port(&cfg, service.clone()).unwrap();
            expect_eq!(Arc::strong_count(&port_wrapper.service_handle), 2);
            expect_eq!(Arc::strong_count(&service), 2);
        }
        expect_eq!(Arc::strong_count(&service), 1);
    }

    #[test]
    fn add_port_drop_wrapper() {
        let handle_set_wrapper = HandleSetWrapper::<()>::new().unwrap();
        let path = format!("{}.port.{}", SRV_PATH_BASE, "test");
        let cfg = PortCfg::new(path).unwrap();
        let service = Arc::new(());
        {
            // This line ignores the returned wrapper, which immediately drops it.
            handle_set_wrapper.add_port(&cfg, service.clone()).unwrap();
            expect_eq!(Arc::strong_count(&service), 1);
        }
        expect_eq!(Arc::strong_count(&service), 1);
    }

    #[test]
    fn add_connection() {
        let handle_set_wrapper = HandleSetWrapper::<()>::new().unwrap();
        let path = format!("{}.port.{}", SRV_PATH_BASE, "test");
        let cfg = PortCfg::new(path).unwrap();
        let service = Arc::new(());

        // SAFETY: syscall, `cfg` is a local and outlives the call.
        // The return value is either a negative error code or a valid handle.
        let rc = unsafe {
            trusty_sys::port_create(
                cfg.get_path().as_ptr(),
                cfg.get_msg_queue_len(),
                cfg.get_msg_max_size(),
                cfg.get_flags(),
            )
        };
        expect!(rc >= 0, "created the connection handle");

        let handle_fd = rc as i32;
        let handle = Handle::from_raw(handle_fd).unwrap();
        handle_set_wrapper.add_connection((), handle, service.clone()).unwrap();
        expect_eq!(Arc::strong_count(&service), 2);
        expect!(handle_set_wrapper.remove_raw(handle_fd).is_ok(), "removed the connection handle");
        expect_eq!(Arc::strong_count(&service), 1);
    }

    #[test]
    fn add_connection_invalid_handle() {
        let handle_set_wrapper = HandleSetWrapper::<()>::new().unwrap();
        let handle = Handle::from_raw(1063).unwrap();
        let service = Arc::new(());

        expect_eq!(
            Err(TipcError::SystemError(trusty_sys::Error::NotFound)),
            handle_set_wrapper.add_connection((), handle, service.clone()),
            "invalid handle"
        );
        expect_eq!(Arc::strong_count(&service), 1);
    }

    #[test]
    fn remove() {
        let handle_set_wrapper = HandleSetWrapper::<()>::new().unwrap();
        let path = format!("{}.port.{}", SRV_PATH_BASE, "test");
        let cfg = PortCfg::new(path).unwrap();
        let service = Arc::new(());
        {
            let port_wrapper = handle_set_wrapper.add_port(&cfg, service.clone()).unwrap();
            expect_eq!(Arc::strong_count(&port_wrapper.service_handle), 2);
            expect_eq!(Arc::strong_count(&service), 2);
            expect!(handle_set_wrapper.remove(port_wrapper.service_handle.clone()).is_ok());
            expect_eq!(Arc::strong_count(&port_wrapper.service_handle), 1);
            expect_eq!(
                Err(TipcError::SystemError(trusty_sys::Error::NotFound)),
                handle_set_wrapper.remove(port_wrapper.service_handle.clone()),
                "handle already removed"
            );
        }
        // `port_wrapper.service_handle` gets removed one more time here
        // when port_wrapper is dropped. This emits an error message.
        expect_eq!(Arc::strong_count(&service), 1);
    }

    #[test]
    fn remove_raw() {
        let handle_set_wrapper = HandleSetWrapper::<()>::new().unwrap();
        let path = format!("{}.port.{}", SRV_PATH_BASE, "test");
        let cfg = PortCfg::new(path).unwrap();
        let service = Arc::new(());
        {
            let port_wrapper = handle_set_wrapper.add_port(&cfg, service.clone()).unwrap();
            expect_eq!(Arc::strong_count(&port_wrapper.service_handle), 2);
            expect_eq!(Arc::strong_count(&service), 2);
            expect!(handle_set_wrapper
                .remove_raw(port_wrapper.service_handle.get_raw_fd_id())
                .is_ok());
            expect_eq!(Arc::strong_count(&port_wrapper.service_handle), 1);
            expect_eq!(
                Err(TipcError::SystemError(trusty_sys::Error::NotFound)),
                handle_set_wrapper.remove(port_wrapper.service_handle.clone())
            );
        }
        // `port_wrapper.service_handle` gets removed one more time here
        // when port_wrapper is dropped. This emits an error message.
        expect_eq!(Arc::strong_count(&service), 1);
    }

    #[test]
    fn wait_on_port() {
        let handle_set_wrapper = HandleSetWrapper::<()>::new().unwrap();
        let mut ports = Vec::new();
        let service = Arc::new(());

        for i in first_free_handle_index()..MAX_USER_HANDLES - 1 {
            let path = format!("{}.port.{}{}", SRV_PATH_BASE, "test", i);
            let cfg = PortCfg::new(path).unwrap();
            let result = handle_set_wrapper.add_port(&cfg, service.clone());
            ports.push(result.unwrap());
        }

        expect_eq!(
            handle_set_wrapper.wait(Some(0)).err(),
            Some(TipcError::SystemError(Error::TimedOut)),
            "zero timeout"
        );
        expect_eq!(
            handle_set_wrapper.wait(Some(100)).err(),
            Some(TipcError::SystemError(Error::TimedOut)),
            "non zero timeout"
        );
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

    enum WrapperService {
        Service1(Service1),
        Service2(Service2),
    }

    impl UnbufferedService for WrapperService {
        type Connection = ();

        fn on_connect(
            &self,
            port: &PortCfg,
            handle: &Handle,
            peer: &Uuid,
        ) -> Result<ConnectResult<Self::Connection>> {
            match self {
                Self::Service1(service_1) => {
                    match UnbufferedService::on_connect(service_1, port, handle, peer) {
                        Ok(conn_result) => match conn_result {
                            ConnectResult::Accept(conn) => Ok(ConnectResult::Accept(conn.into())),
                            ConnectResult::CloseConnection => Ok(ConnectResult::CloseConnection),
                        },
                        Err(e) => Err(e),
                    }
                }

                Self::Service2(service_2) => {
                    match UnbufferedService::on_connect(service_2, port, handle, peer) {
                        Ok(conn_result) => match conn_result {
                            ConnectResult::Accept(conn) => Ok(ConnectResult::Accept(conn.into())),
                            ConnectResult::CloseConnection => Ok(ConnectResult::CloseConnection),
                        },
                        Err(e) => Err(e),
                    }
                }
            }
        }

        fn on_message(
            &self,
            connection: &Self::Connection,
            handle: &Handle,
            buffer: &mut [u8],
        ) -> Result<MessageResult> {
            match self {
                Self::Service1(service_1) => UnbufferedService::on_message(
                    service_1,
                    connection.try_into().map_err(|_| TipcError::InvalidData)?,
                    handle,
                    buffer,
                ),
                Self::Service2(service_2) => UnbufferedService::on_message(
                    service_2,
                    connection.try_into().map_err(|_| TipcError::InvalidData)?,
                    handle,
                    buffer,
                ),
            }
        }
    }

    #[test]
    fn multiple_services() {
        let handle_set_wrapper = Arc::new(HandleSetWrapper::new().unwrap());

        let path_1 = format!("{}.port.{}", SRV_PATH_BASE, "testService1");
        let cfg_1 = PortCfg::new(&path_1).unwrap();
        let service_1 = Arc::new(WrapperService::Service1(Service1));

        let path_2 = format!("{}.port.{}", SRV_PATH_BASE, "testService2");
        let cfg_2 = PortCfg::new(&path_2).unwrap();
        let service_2 = Arc::new(WrapperService::Service2(Service2));

        {
            let port_wrapper_1 =
                handle_set_wrapper.add_port(&cfg_1, Arc::clone(&service_1)).unwrap();
            let port_wrapper_2 =
                handle_set_wrapper.add_port(&cfg_2, Arc::clone(&service_2)).unwrap();

            expect_eq!(Arc::strong_count(&port_wrapper_1.service_handle), 2);
            expect_eq!(Arc::strong_count(&service_1), 2);

            expect_eq!(Arc::strong_count(&port_wrapper_2.service_handle), 2);
            expect_eq!(Arc::strong_count(&service_2), 2);
        }

        expect_eq!(Arc::strong_count(&service_1), 1);
        expect_eq!(Arc::strong_count(&service_2), 1);
    }
}
