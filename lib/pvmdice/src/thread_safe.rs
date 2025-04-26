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

//! This module provides PvmDiceThreadSafe.
//!
//! This wrapper was originally written to support a single PvmDice object being
//! shared between an hwbcc server and an rpcbinder server running in the same
//! TA and event loop. rpcbinder services are thread safe and so we need a version
//! of PvmDice that is Send + Sync if we want to use it within the request path
//! of one of those services.
//!
//! Trusty user space does not support multi-threading so we assume a Mutex will
//! never be poisoned and call unwrap throughout.

use crate::PvmDice;
use hwbcc::srv::{HwBccOps, RequestContext};
use hwbcc::{HwBccError, HwBccMode};
use std::sync::{Arc, Mutex};

pub struct PvmDiceThreadSafe {
    pvmdice: Arc<Mutex<PvmDice>>,
}

impl PvmDiceThreadSafe {
    pub fn new(pvmdice: Arc<Mutex<PvmDice>>) -> Self {
        PvmDiceThreadSafe { pvmdice }
    }
}

impl HwBccOps for PvmDiceThreadSafe {
    fn init(&self, context: &RequestContext) -> Result<(), HwBccError> {
        self.pvmdice.lock().unwrap().init(context)
    }

    fn close(&self, context: &RequestContext) {
        self.pvmdice.lock().unwrap().close(context)
    }

    fn get_bcc(
        &self,
        request_ctx: &RequestContext,
        mode: HwBccMode,
    ) -> Result<Vec<u8>, HwBccError> {
        self.pvmdice.lock().unwrap().get_bcc(request_ctx, mode)
    }

    fn sign_data<'a>(
        &self,
        request_ctx: &RequestContext,
        data: &'a [u8],
        aad: &'a [u8],
        mode: HwBccMode,
    ) -> Result<Vec<u8>, HwBccError> {
        self.pvmdice.lock().unwrap().sign_data(request_ctx, data, aad, mode)
    }
}
