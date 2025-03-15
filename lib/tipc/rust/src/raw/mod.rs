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

///! Entry point to a new tipc user space API. The goal of this module are three folds:
///  1. the ability to share the handleset with the event loop as well as a
///     service
///  2. removing the dispatcher(present in the legacy Rust tipc library) from the event pipeline
///  3. make the user space constructs thread safe, so that they can be shared with RPC binder
///     services which has the thread safety requirement
///
///  A TA initialization code may use this API in the following manner to get the advantage of
///  the aforementioned features.
///  1. Create a `HandleSetWrapper` that manages a particular type of handle objects in an instance
///     of a `RawHandleSet`.
///  2. Create a tipc service that implements a given `*Service` trait
///  3. Create a port type handle object containing a reference to the service and register it in
///     the `HandleSetWrapper`.
///  4. Give a reference of the `HandleSetWrapper` to the service, if the service needs to
///     manipulate the `HandleSetWrapper`.
///  5. Create the `EventLoop` with a reference to the HandleSetWrapper and run the `EventLoop`.
///
///  Note: This crate currently accommodates the services that implement the`UnbufferredService`
///  trait only. It is a future work to implement a common `HandleSetWrapper` that can accommodate
///  the services that implement the `Service` trait.
mod event_loop;
mod handle_set_wrapper;
mod raw_handle_set;
mod service_handle;

pub use event_loop::EventLoop;
pub use handle_set_wrapper::{HandleSetWrapper, ToConnect, WorkToDo};
pub use raw_handle_set::{Handler, RawHandleSet};
pub use service_handle::{HandleType, ServiceHandle};
