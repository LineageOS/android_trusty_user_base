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

//! Defines the event loop runner for the TAs. Compared to the legacy event loop included in the
//! `Manager`, this does not use `Dispatcher`.
use crate::raw::{HandleSetWrapper, HandleType};
use crate::sys;
use crate::{MessageResult, Result, TipcError, UnbufferedService};
use alloc::sync::Arc;
use log::{debug, error, warn};

/// The event loop runner that waits for and dispatches the events.
pub struct EventLoop<S: UnbufferedService> {
    handle_set_wrapper: Arc<HandleSetWrapper<S>>,
}

impl<S: UnbufferedService> EventLoop<S> {
    pub fn new(handle_set_wrapper: Arc<HandleSetWrapper<S>>) -> Self {
        Self { handle_set_wrapper }
    }

    /// Runs the event loop.
    pub fn run(&self) -> Result<()> {
        loop {
            let err_result = match self.wait_and_handle_event() {
                Ok(()) => continue,
                Err(e) => e,
            };
            // TODO(b/382291660): refactor
            // The following code is equivalent to the error handling in:
            // trusty/user/lib/tipc/rust/src/service.rs->run_event_loop()
            // Check if the error is recoverable or not. If the error is not one of a
            // limited set of recoverable errors, we break from the event loop.
            use trusty_sys::Error;
            match err_result {
                // Recoverable errors that are always ignored.
                | TipcError::SystemError(Error::TimedOut)
                | TipcError::SystemError(Error::ChannelClosed)

                // returned when peer UUID connection is not allowed.
                | TipcError::SystemError(Error::NotAllowed)

                // These are always caused by the client and so shouldn't be treated as an
                // internal error or cause the event loop to exit.
                | TipcError::ChannelClosed
                => {
                    debug!("Recoverable error ignored: {:?}", err_result)
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
                        err_result,
                    )
                }

                _ => {
                    error!("Error occurred while handling incoming event: {:?}", err_result);
                    return Err(err_result);
                }
            }
        }
    }

    fn wait_and_handle_event(&self) -> Result<()> {
        let (service_handle, event) = self.handle_set_wrapper.wait(None)?;
        // TODO(b/382291660): refactor, the code should handle all events, not just one
        // TODO(b/382291660): Abort on port errors?
        // The following code is equivalent to: trusty/user/lib/tipc/rust/src/service.rs->handler()
        match &service_handle.ty {
            HandleType::Port(_) if event & (sys::IPC_HANDLE_POLL_READY as u32) != 0 => {
                self.handle_set_wrapper.handle_connect(service_handle)
            }
            HandleType::Connection(_) if event & (sys::IPC_HANDLE_POLL_MSG as u32) != 0 => {
                match self.handle_set_wrapper.handle_message(Arc::clone(&service_handle)) {
                    Ok(MessageResult::MaintainConnection) => Ok(()),
                    Ok(MessageResult::CloseConnection) => {
                        self.handle_set_wrapper.remove(service_handle)?;
                        Ok(())
                    }
                    Err(e) => {
                        error!("Could not handle message, closing connection: {:?}", e);
                        self.handle_set_wrapper.remove(service_handle)?;
                        Ok(())
                    }
                }
            }
            HandleType::Connection(_) if event & (sys::IPC_HANDLE_POLL_HUP as u32) != 0 => {
                self.handle_set_wrapper.handle_disconnect(service_handle)
            }

            //TODO(b/382291660): update this comment
            // `SEND_UNBLOCKED` means that some previous attempt to send a message was
            // blocked and has now become unblocked. This should normally be handled by
            // the code trying to send the message, but if the sending code doesn't do so
            // then we can end up getting it here.
            _ if event & (sys::IPC_HANDLE_POLL_SEND_UNBLOCKED as u32) != 0 => {
                warn!(
                    "Received `SEND_UNBLOCKED` event received in main event loop. \
                     This likely means that a sent message was lost somewhere"
                );
                //TODO(b/382291660): it is not safe to ignore this error
                Ok(())
            }

            // `NONE` is not an event we should get in practice, but if it does then it
            // shouldn't trigger an error.
            _ if event == (sys::IPC_HANDLE_POLL_NONE as u32) => Ok(()),

            // Treat any unrecognized events as errors by default.
            _ => {
                error!("Could not handle event {}", event);
                Err(TipcError::UnknownError)
            }
        }
    }
}
