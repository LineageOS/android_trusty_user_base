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

//! # Integration tests for the trusty service_manager lib.
//!
//!
//! ## Test components
//!
//! ### ISMTestService
//! The AIDL definition for the test service we use in this test suite.
//!
//! ### service_manager_test_service
//! This is a trusty user space app that implements ISMTestService.
//! It exposes "com.android.trusty.test_service.ISMTestService/direct" which, allows direct
//! access to the test service.
//!
//! ### fake_accessor
//! This is a trusty user space app that implements a binder ITrustyAccessor interface that returns
//! a pre-connected fd to the service_manager_test_service. It exposes this accessor implementation
//! on "com.android.trusty.test_service.ISMTestService/through_accessor".
//!
//! fake_accessor also exposes "com.android.trusty.test_service.ISMTestService/expect_mismatch" that
//! intentionally has a different instance name than the port on which it is served. This allows us
//! to cover this error condition in our integration tests.
//!

#[cfg(test)]
mod tests {
    use service_manager::*;
    use service_manager_test_service::binder;
    use service_manager_test_service::aidl::com::android::trusty::test_service::ISMTestService::ISMTestService;
    use android_hardware_security_see_storage::aidl::android::hardware::security::see::storage::ISecureStorage::ISecureStorage;
    use test::{expect, expect_eq};

    test::init!();

    #[test]
    fn test_wait_for_interface_direct() {
        let test_service: Result<binder::Strong<dyn ISMTestService>, binder::StatusCode> =
            wait_for_interface("com.android.trusty.test_service.ISMTestService/direct");
        expect!(test_service.is_ok());
        expect!(test_service.unwrap().hello().is_ok());
    }

    #[test]
    fn test_wait_for_interface_through_accessor() {
        let test_service: Result<binder::Strong<dyn ISMTestService>, binder::StatusCode> =
            wait_for_interface("com.android.trusty.test_service.ISMTestService/accessor");
        expect!(test_service.is_ok());
        expect!(test_service.unwrap().hello().is_ok());
    }

    #[test]
    fn test_accessor_mismatch() {
        let test_service: Result<binder::Strong<dyn ISMTestService>, binder::StatusCode> =
            wait_for_interface("com.android.trusty.test_service.ISMTestService/mismatch");
        expect_eq!(test_service, Err(binder::StatusCode::NAME_NOT_FOUND));
    }

    #[test]
    fn test_connection_failure() {
        // tipc::Handle::connect will block forever on a non-existent port so a good way to
        // get a connection failure is to connect to a real port that we don't have permissions for.
        let test_service: Result<binder::Strong<dyn ISMTestService>, binder::StatusCode> =
            wait_for_interface("com.android.trusty.test_service.ISMTestService/eperm");

        expect_eq!(test_service, Err(binder::StatusCode::PERMISSION_DENIED));
    }

    #[test]
    fn test_wrong_direct_interface_requested() {
        let test_service: Result<binder::Strong<dyn ISecureStorage>, binder::StatusCode> =
            wait_for_interface("com.android.trusty.test_service.ISMTestService/direct");
        expect_eq!(test_service, Err(binder::StatusCode::BAD_TYPE));
    }

    #[test]
    fn test_wrong_interface_requested_through_accessor() {
        let test_service: Result<binder::Strong<dyn ISecureStorage>, binder::StatusCode> =
            wait_for_interface("com.android.trusty.test_service.ISMTestService/accessor");
        expect_eq!(test_service, Err(binder::StatusCode::BAD_TYPE));
    }

    #[test]
    fn test_service_name_to_trusty_port_under_max() {
        expect_eq!(
            service_name_to_trusty_port("foo.bar.ok.IShortService/default"),
            Ok("foo.bar.ok.IShortService/default.bnd".to_owned())
        )
    }

    #[test]
    fn test_service_name_to_trusty_port_too_long_known_prefix() {
        expect_eq!(
            service_name_to_trusty_port(
                "android.hardware.security.see.authmgr.IAuthMgrAuthorization/default"
            ),
            Ok("ahss.authmgr.IAuthMgrAuthorization/default.bnd".to_owned()),
        )
    }

    #[test]
    fn test_service_name_to_trusty_port_too_long_invalid() {
        expect_eq!(
            service_name_to_trusty_port(
                "unknown.prefix.so.we.dont.handle.it.for.now.ILongServiceName/default"
            ),
            Err(binder::StatusCode::BAD_VALUE),
        )
    }

    #[test]
    fn test_handover_service_name_to_trusty_port_under_max_without_trustedhal_prefix() {
        expect_eq!(
            service_name_to_trusty_port("IHandover/android.hardware.security.IHelloWorld/default"),
            Ok("android.hardware.security.IHelloWorld/default.hnd".to_owned()),
        );
    }

    #[test]
    fn test_handover_service_name_to_trusty_port_under_max_with_trustedhal_prefix() {
        expect_eq!(
            service_name_to_trusty_port(
                "IHandover/android.hardware.security.see.hwcrypto.IHwCryptoKey/default"
            ),
            Ok("android.hardware.security.see.hwcrypto.IHwCryptoKey/default.hnd".to_owned()),
        )
    }

    #[test]
    fn test_handover_service_name_to_trusty_port_too_long_without_trustedhal_prefix() {
        expect_eq!(
            service_name_to_trusty_port(
                "IHandover/android.hardware.some.too.long.path.secure.ISecureStorage/default"
            ),
            Err(binder::StatusCode::BAD_VALUE),
        )
    }

    #[test]
    fn test_handover_service_name_to_trusty_port_too_long_with_trustedhal_prefix() {
        expect_eq!(
            service_name_to_trusty_port(
                "IHandover/android.hardware.security.see.secure.storage.ISecureStorage/default"
            ),
            Ok("ahss.secure.storage.ISecureStorage/default.hnd".to_owned()),
        )
    }

    #[test]
    fn test_handover_service_name_to_trusty_port_too_long_with_wrong_pattern() {
        expect_eq!(
            service_name_to_trusty_port(
                "wrong.IHandover/hardware.security.secure.storage.ISecureStorage/default"
            ),
            Err(binder::StatusCode::BAD_VALUE),
        );

        expect_eq!(
            service_name_to_trusty_port(
                "android.hardware.some.too.long.path.ISecureStorage/default/IHandover/"
            ),
            Err(binder::StatusCode::BAD_VALUE),
        )
    }
}
