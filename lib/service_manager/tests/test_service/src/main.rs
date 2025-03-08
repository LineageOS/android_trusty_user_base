use alloc::rc::Rc;
use binder::{BinderFeatures, Status};
use rpcbinder::RpcServer;
use service_manager_test_service::aidl::com::android::trusty::test_service::ISMTestService::{
    BnSMTestService, ISMTestService,
};
use tipc::{service_dispatcher, wrap_service, Manager, PortCfg};

const DIRECT_TEST_SERVICE_PORT: &str = "com.android.trusty.test_service.ISMTestService/direct.bnd";

struct TestService;

impl binder::Interface for TestService {}
impl ISMTestService for TestService {
    fn hello(&self) -> Result<String, Status> {
        Ok("Hello from the service manager test service!".to_owned())
    }
}

wrap_service!(DirectTestService(RpcServer: UnbufferedService));

service_dispatcher! {
    enum TestServices {
        DirectTestService,
    }
}

fn main() {
    trusty_log::init();
    let direct_service = BnSMTestService::new_binder(TestService, BinderFeatures::default());
    let direct_rpc_server =
        RpcServer::new_per_session(move |_uuid| Some(direct_service.as_binder()));
    let direct = DirectTestService(direct_rpc_server);

    let mut dispatcher = TestServices::<1>::new().expect("Failed to create dispatcher");

    let cfg = PortCfg::new(DIRECT_TEST_SERVICE_PORT)
        .expect("could not create port config")
        .allow_ta_connect();

    dispatcher
        .add_service(Rc::new(direct), cfg)
        .expect("failed to add direct test service to dispatcher");

    Manager::<_, _, 1, 1>::new_with_dispatcher(dispatcher, [])
        .expect("Service manager could not be created")
        .run_event_loop()
        .expect("Service manager test service failed");
}
