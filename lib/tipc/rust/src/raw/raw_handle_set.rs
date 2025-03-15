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

//! Define the raw handle set which registers in and removes from the handle set a given type of a
//! handle. It waits on all the registered handles and returns the handle object along with the
//! event when an event takes place in any of the handles.
use crate::sys;
use crate::{Handle, Result, TipcError};
use alloc::sync::Arc;
use core::ffi::c_void;
use core::marker::PhantomData;

/// Trait that should be implemented by a handle object that is passed into the `RawHandleSet`.
pub trait Handler {
    // Get the raw file descriptor corresponding to the handle object.
    fn get_raw_fd_id(&self) -> i32;
}

/// Raw handle set parameterized by the type of the handle stored in the event cookie.
pub struct RawHandleSet<H: Handler> {
    // The handle that represents the handleset
    handle: Handle,
    data: PhantomData<H>,
}

impl<H: Handler> RawHandleSet<H> {
    /// Constructor
    pub fn new() -> Result<Self> {
        // SAFETY: syscall, return value is either a negative error code or a
        // valid handle.
        let rc = unsafe { trusty_sys::handle_set_create() };
        if rc < 0 {
            Err(TipcError::from_uapi(rc))
        } else {
            let handle = Handle::from_raw(rc as i32)?;
            Ok(RawHandleSet::<H> { handle, data: PhantomData })
        }
    }

    /// Waits on the handles for events and returns the handle object that was registered in the
    /// event cookie and the event flags.
    pub fn wait(&self, timeout: Option<u32>) -> Result<(Arc<H>, u32)> {
        let event = self.handle.wait(timeout)?;
        let ptr = event.cookie;
        let handle_obj =
            // SAFETY: the `ptr` is the `Arc` that we registered in `register`. If we receive
            // an event on a handle with this cookie, we return an `Arc` with a bumped ref count.
            unsafe {
                Arc::increment_strong_count(ptr);
                Arc::from_raw(ptr.cast::<H>())
            };
        Ok((handle_obj, event.event))
    }

    /// Register a handle object as the event cookie along with the corresponding Handle.
    pub fn register(&self, event_cookie_obj: Arc<H>) -> Result<()> {
        let ret = self.do_set_ctrl(
            sys::HSET_ADD as u32,
            trusty_sys::uevent::ALL_EVENTS,
            &event_cookie_obj,
        );
        if ret.is_ok() {
            std::mem::forget(event_cookie_obj);
        }
        ret
    }

    /// Remove a previously registered handle object.
    pub fn remove(&self, event_cookie_obj: Arc<H>) -> Result<()> {
        self.remove_raw(event_cookie_obj.get_raw_fd_id()).map(|_| ())
    }

    /// Remove a previously registered handle object and return an arc to it.
    pub fn remove_raw(&self, handle: i32) -> Result<Arc<H>> {
        let mut uevt = trusty_sys::uevent { handle, event: 0, cookie: std::ptr::null_mut() };
        // SAFETY: syscall. The uevent pointer points to a correctly initialized
        // structure that is valid across the call. The handle for the handle set is valid for
        // the same lifetime as self, so will remain valid at least as long as the handle object
        // being added/modified.
        let rc = unsafe {
            trusty_sys::handle_set_ctrl(
                self.handle.as_raw_fd(),
                sys::HSET_DEL_GET_COOKIE,
                &mut uevt,
            )
        };

        if rc < 0 {
            Err(TipcError::from_uapi(rc))
        } else {
            // SAFETY: the `cookie` is the `Arc` that we registered in `register`. Here, we
            // successfully deleted the handle from the handle set and are returning its cookie.
            unsafe { Ok(Arc::from_raw(uevt.cookie.cast::<H>())) }
        }
    }

    // Add, update or remove the handle object (event cookie object) from the kernel handle set.
    // This function should never be public.
    // When this is called in the delete path, an arc to the deleted handle object is returned to
    // caller, without dropping it, because the caller may need it for further processing.
    fn do_set_ctrl(&self, cmd: u32, event: u32, event_cookie_obj: &Arc<H>) -> Result<()> {
        let raw_handle_fd: i32 = event_cookie_obj.get_raw_fd_id();
        let cookie = Arc::as_ptr(event_cookie_obj);
        let mut uevt =
            trusty_sys::uevent { handle: raw_handle_fd, event, cookie: cookie as *mut c_void };

        // SAFETY: syscall. The uevent pointer points to a correctly initialized
        // structure that is valid across the call. The handle for the handle set is valid for
        // the same lifetime as self, so will remain valid at least as long as the handle object
        // being added/modified.
        let rc = unsafe { trusty_sys::handle_set_ctrl(self.handle.as_raw_fd(), cmd, &mut uevt) };

        if rc < 0 {
            Err(TipcError::from_uapi(rc))
        } else {
            Ok(())
        }
    }
}
