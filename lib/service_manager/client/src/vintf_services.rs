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

//! This module contains helpers for trusty TAs that need to provide or test against
//! VINTF stable services in trusty. It's pretty unlikely that you should be using
//! these helpers unless you're implementing a TA that facilitates access to services
//! like authmgr-fe or the VINTF testing TA.
//!
//! To get an SpIBinder for each supported VINTF services, you can do the following:
//!
//! ```
//! use service_manager::vintf_services::{VintfServiceImpl, get_supported_vintf_services};
//!
//! for service in get_supported_vintf_services() {
//!     binder = service.get_binder();
//!     // use `binder` here
//! }
//! ```

use binder::{SpIBinder, FromIBinder};
use crate::wait_for_interface;
use android_hardware_security_see_authmgr::aidl::android::hardware::security::see::authmgr::IAuthMgrAuthorization::IAuthMgrAuthorization;
use android_hardware_security_see_hwcrypto::aidl::android::hardware::security::see::hwcrypto::IHwCryptoKey::IHwCryptoKey;
use android_hardware_security_see_storage::aidl::android::hardware::security::see::storage::ISecureStorage::ISecureStorage;
use std::marker::PhantomData;

pub trait VintfService {
    /// Get the root SpIBinder for this VINTF service.
    /// NOTE: the only implementation of this trait will block
    /// forever if no service is published at the expected port.
    fn get_binder(&self) -> binder::Result<SpIBinder>;

    /// The name for this service. This name is used to
    /// publish the service locally. Callers should not depend
    /// on it to derive class and instance names, but should
    /// get this information from the binder object itself.
    fn name(&self) -> &'static str;
}

pub struct VintfServiceImpl<T: FromIBinder + ?Sized> {
    name: &'static str,
    phantom: PhantomData<T>,
    unimplemented: bool,
}

impl<T: FromIBinder + ?Sized> VintfServiceImpl<T> {
    fn new(name: &'static str) -> Self {
        Self { name, phantom: PhantomData, unimplemented: false }
    }

    // TODO: b/408414703 - remove once IAuthMgrAuthorization is exposed by authmgr-fe
    // An implementation of VintfService that will fail when `get_binder` is
    // called. For services that are not yet implemented, this is preferable
    // to hanging forever waiting for an unpublished port.
    fn new_unimplemented(name: &'static str) -> Self {
        Self { name, phantom: PhantomData, unimplemented: true }
    }
}

impl<T: FromIBinder + ?Sized> VintfService for VintfServiceImpl<T> {
    fn get_binder(&self) -> binder::Result<SpIBinder> {
        if self.unimplemented {
            return Err(binder::Status::new_exception_str(
                binder::ExceptionCode::UNSUPPORTED_OPERATION,
                Some("VINTF service is not yet implemented"),
            ));
        }

        Ok(wait_for_interface::<T>(self.name)?.as_binder())
    }

    fn name(&self) -> &'static str {
        self.name
    }
}

/// Get all supported VINTF services.
pub fn get_supported_vintf_services() -> Vec<Box<dyn VintfService>> {
    vec![
        Box::new(VintfServiceImpl::<dyn ISecureStorage>::new(
            "android.hardware.security.see.storage.ISecureStorage/default",
        )),
        Box::new(VintfServiceImpl::<dyn IHwCryptoKey>::new(
            "android.hardware.security.see.hwcrypto.IHwCryptoKey/default",
        )),
        Box::new(VintfServiceImpl::<dyn IAuthMgrAuthorization>::new_unimplemented(
            "android.hardware.security.see.authmgr.IAuthMgrAuthorization/default",
        )),
    ]
}
