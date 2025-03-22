/*
 * Copyright (C) 2022 The Android Open Source Project
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

use alloc::collections::TryReserveError;
use core::num::TryFromIntError;
use diced_open_dice::DiceError;
use tipc::TipcError;
use trusty_std::alloc::AllocError;
use trusty_sys::Error;

#[derive(Debug, PartialEq)]
pub enum HwBccError {
    NotAllowed,
    BadLen,
    OutOfBounds,
    AllocError,
    InvalidCmdResponse,
    TryFromIntError,
    Tipc(TipcError),
    System(Error),
}

impl From<TipcError> for HwBccError {
    fn from(err: TipcError) -> Self {
        HwBccError::Tipc(err)
    }
}

impl From<Error> for HwBccError {
    fn from(err: Error) -> Self {
        HwBccError::System(err)
    }
}

impl From<TryFromIntError> for HwBccError {
    fn from(_err: TryFromIntError) -> Self {
        HwBccError::OutOfBounds
    }
}

impl From<AllocError> for HwBccError {
    fn from(_err: AllocError) -> Self {
        HwBccError::AllocError
    }
}

impl From<TryReserveError> for HwBccError {
    fn from(_err: TryReserveError) -> Self {
        HwBccError::AllocError
    }
}

impl From<DiceError> for HwBccError {
    fn from(dice_error: DiceError) -> Self {
        match dice_error {
            DiceError::InvalidInput => HwBccError::System(trusty_sys::Error::InvalidArgs),
            DiceError::BufferTooSmall(_) => HwBccError::System(trusty_sys::Error::NotEnoughBuffer),
            DiceError::PlatformError => HwBccError::System(trusty_sys::Error::Generic),
            DiceError::UnsupportedKeyAlgorithm(_) => {
                HwBccError::System(trusty_sys::Error::NotSupported)
            }
            DiceError::MemoryAllocationError => HwBccError::System(trusty_sys::Error::NoMemory),
            DiceError::DiceChainNotFound => HwBccError::System(trusty_sys::Error::InvalidArgs),
        }
    }
}
