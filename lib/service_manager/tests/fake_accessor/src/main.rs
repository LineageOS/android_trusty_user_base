use alloc::rc::Rc;
use binder::{BinderFeatures, Interface, ParcelFileDescriptor, Status};
use rpcbinder::RpcServer;
use service_manager::{service_name_to_trusty_c_port, service_name_to_trusty_port};
use std::os::fd::{FromRawFd, OwnedFd};
use tipc::{service_dispatcher, wrap_service, Manager, PortCfg};
use trusty_binder_accessor::aidl::trusty::os::ITrustyAccessor::{
    BnTrustyAccessor, ITrustyAccessor, ERROR_FAILED_TO_CREATE_SOCKET,
};
const TEST_SERVICE_DIRECT_NAME: &str = "com.android.trusty.test_service.ISMTestService/direct";
// This is the port that the accessor is exposing to the rest of the user space TAs
const TEST_SERVICE_PUBLIC_NAME: &str = "com.android.trusty.test_service.ISMTestService/accessor";
// This port exists to allow tests to exercise a check for the correct instance name from an accessor.
// When connecting to this port, clients will find an ITrustyAccessor whose instance name does not match the
// port name.
const ACCESSOR_MISMATCH_TEST_NAME: &str = "com.android.trusty.test_service.ISMTestService/mismatch";
// This port is locked down to not allow access to any existing TAs. It allows the client tests
// to exercise connection failure paths.
const ACCESSOR_NO_PERMISSIONS: &str = "com.android.trusty.test_service.ISMTestService/eperm";

struct FakeAccessor;
impl Interface for FakeAccessor {}
impl ITrustyAccessor for FakeAccessor {
    fn addConnection(&self) -> Result<ParcelFileDescriptor, Status> {
        let port = service_name_to_trusty_c_port(TEST_SERVICE_DIRECT_NAME)?;
        let handle = tipc::Handle::connect(port.as_c_str()).map_err(|_| {
            binder::Status::new_service_specific_error(
                ERROR_FAILED_TO_CREATE_SOCKET,
                Some(c"Failed to connect to com.android.trusty.test_service.ISMTestService/direct.bnd"),
            )
        })?;
        let fd = handle.as_raw_fd();
        // Do not close this fd. We're passing ownership of it
        // to ParcelFileDescriptor.
        core::mem::forget(handle);
        // SAFETY: The fd is open since it was obtained from a successful call to
        // tipc::Handle::connect. The fd is suitable for transferring ownership because we've leaked
        // the original handle to ensure it isn't dropped.
        let owned_fd = unsafe { OwnedFd::from_raw_fd(fd) };
        Ok(ParcelFileDescriptor::new(owned_fd))
    }
    fn getInstanceName(&self) -> Result<String, Status> {
        Ok(TEST_SERVICE_PUBLIC_NAME.to_owned())
    }
}
fn new_fake_accessor_server() -> RpcServer {
    let binder = BnTrustyAccessor::new_binder(FakeAccessor, BinderFeatures::default());
    RpcServer::new_per_session(move |_uuid| Some(binder.as_binder()))
}
wrap_service!(AccessorExposingTestService(RpcServer: UnbufferedService));
service_dispatcher! {
    enum AccessorServices {
        AccessorExposingTestService,
    }
}
fn main() {
    log::info!("Starting fake accessor...");
    let test_service_server = new_fake_accessor_server();
    let test_service = Rc::new(AccessorExposingTestService(test_service_server));
    let mut dispatcher = AccessorServices::<3>::new().expect("Failed to create dispatcher");
    let cfg = PortCfg::new(
        service_name_to_trusty_port(TEST_SERVICE_PUBLIC_NAME)
            .expect("test service port to be resolved"),
    )
    .expect("could not create port config")
    .allow_ta_connect();
    dispatcher
        .add_service(test_service.clone(), cfg)
        .expect("failed to add accessor test service to dispatcher");
    let cfg = PortCfg::new(
        service_name_to_trusty_port(ACCESSOR_MISMATCH_TEST_NAME)
            .expect("accessor mismatch port to be resolved"),
    )
    .expect("could not create port config")
    .allow_ta_connect();
    dispatcher
        .add_service(test_service.clone(), cfg)
        .expect("failed to add mismatch test port to dispatcher");

    // Purposefully do not set the ta or ns allow flags.
    // If we reject by UUID allow list instead, there is a race condition
    // between the connection being accepted and then subsequently dropped
    // when the permission check happens in user space on the server.
    // This is typically fine, but leads to non-deterministic behavior in
    // tests that are trying to test rejected connections.
    let cfg = PortCfg::new(
        service_name_to_trusty_port(ACCESSOR_NO_PERMISSIONS).expect("no perm port to be resolved"),
    )
    .expect("could not create port config");
    dispatcher.add_service(test_service, cfg).expect("failed to add eperm test port to dispatcher");
    Manager::<_, _, 3, 1>::new_with_dispatcher(dispatcher, [])
        .expect("Service manager could not be created")
        .run_event_loop()
        .expect("Main event loop exited");
}
