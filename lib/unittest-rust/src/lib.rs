/*
 * This file is partially derived from src/lib.rs in the Rust test library, used
 * under the Apache License, Version 2.0. The following is the original
 * copyright information from the Rust project:
 *
 * Copyrights in the Rust project are retained by their contributors. No
 * copyright assignment is required to contribute to the Rust project.
 *
 * Some files include explicit copyright notices and/or license notices.
 * For full authorship information, see the version control history or
 * https://thanks.rust-lang.org
 *
 * Except as otherwise noted (below and/or in individual files), Rust is
 * licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
 * <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 * <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
 *
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

//! # Trusty Rust Testing Framework

use core::cell::RefCell;
use libc::{clock_gettime, CLOCK_BOOTTIME};
use log::{Log, Metadata, Record};
use tipc::{
    ConnectResult, Handle, Manager, MessageResult, PortCfg, Serialize, Serializer, Service, Uuid,
};
use trusty_log::{TrustyLogger, TrustyLoggerConfig};
use trusty_std::alloc::Vec;

// Public reexports
pub use self::bench::Bencher;
pub use self::options::{ColorConfig, Options, OutputFormat, RunIgnored, ShouldPanic};
pub use self::types::TestName::*;
pub use self::types::*;

#[doc(hidden)]
pub mod __internal_macro_utils {
    pub use crate::{asserts::*, display::*};
}

mod asserts;
mod bench;
mod context;
mod display;
mod macros;
mod options;
mod stats;
mod types;

use context::CONTEXT;

extern "Rust" {
    static TEST_PORT: &'static str;
}

fn get_time_ns() -> u64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };

    // Safety: Passing valid pointer to variable ts which lives past end of call
    unsafe { clock_gettime(CLOCK_BOOTTIME, &mut ts) };

    ts.tv_sec as u64 * 1_000_000_000u64 + ts.tv_nsec as u64
}

/// Initialize a test service for this crate.
///
/// Including an invocation of this macro exactly once is required to configure a
/// crate to set up the Trusty Rust test framework.
///
/// # Examples
///
/// ```
/// #[cfg(test)]
/// mod test {
///     // Initialize the test framework
///     test::init!();
///
///     #[test]
///     fn test() {}
/// }
/// ```
#[macro_export]
macro_rules! init {
    () => {
        #[cfg(test)]
        #[used]
        #[no_mangle]
        pub static TEST_PORT: &'static str = env!(
            "TRUSTY_TEST_PORT",
            "Expected TRUSTY_TEST_PORT environment variable to be set during compilation",
        );
    };
}

// TestMessage::Message doesn't have a use yet
#[allow(dead_code)]
enum TestMessage<'m> {
    Passed,
    Failed,
    Message(&'m str),
}

impl<'m, 's> Serialize<'s> for TestMessage<'m> {
    fn serialize<'a: 's, S: Serializer<'s>>(
        &'a self,
        serializer: &mut S,
    ) -> Result<S::Ok, S::Error> {
        match self {
            TestMessage::Passed => serializer.serialize_bytes(&[0u8]),
            TestMessage::Failed => serializer.serialize_bytes(&[1u8]),
            TestMessage::Message(msg) => {
                serializer.serialize_bytes(&[2u8])?;
                serializer.serialize_bytes(msg.as_bytes())
            }
        }
    }
}

pub struct TrustyTestLogger {
    stderr_logger: TrustyLogger,
    client_connection: RefCell<Option<Handle>>,
}

// SAFETY: This is not actually thread-safe, but we don't implement Mutex in
// Trusty's std yet.
unsafe impl Sync for TrustyTestLogger {}

impl TrustyTestLogger {
    const fn new() -> Self {
        Self {
            stderr_logger: TrustyLogger::new(TrustyLoggerConfig::new()),
            client_connection: RefCell::new(None),
        }
    }

    /// Connect a new client to this logger, disconnecting the existing client,
    /// if any.
    fn connect(&self, handle: &Handle) -> tipc::Result<()> {
        let _ = self.client_connection.replace(Some(handle.try_clone()?));
        Ok(())
    }

    /// Disconnect the current client, if connected.
    ///
    /// If there is not a current client, this method does nothing.
    fn disconnect(&self) {
        let _ = self.client_connection.take();
    }
}

impl Log for TrustyTestLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.stderr_logger.enabled(metadata)
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        self.stderr_logger.log(record);
        if let Some(client) = self.client_connection.borrow().as_ref() {
            let err = if let Some(msg) = record.args().as_str() {
                // avoid an allocation if message is a static str
                client.send(&TestMessage::Message(msg))
            } else {
                let msg = format!("{}\n", record.args());
                client.send(&TestMessage::Message(&msg))
            };
            if let Err(e) = err {
                eprintln!("Could not send log message to test client: {:?}", e);
            }
        }
    }

    fn flush(&self) {
        self.stderr_logger.flush()
    }
}

static LOGGER: TrustyTestLogger = TrustyTestLogger::new();

fn print_status(test: &TestDesc, msg: &str) {
    log::info!("[ {} ] {}", msg, test.name);
}

fn print_status_with_duration(test: &TestDesc, msg: &str, duration_ms: u64) {
    log::info!("[ {} ] {} ({} ms)", msg, test.name, duration_ms);
}

struct TestService {
    tests: Vec<TestDescAndFn>,
}

#[cfg(not(feature = "machine_readable"))]
fn print_samples(_test: &TestDesc, bs: &bench::BenchSamples) {
    use core::fmt::Write;

    struct FmtCounter {
        chars: usize,
    }

    impl FmtCounter {
        fn new() -> FmtCounter {
            FmtCounter { chars: 0 }
        }
    }

    impl core::fmt::Write for FmtCounter {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            self.chars += s.chars().count();
            Ok(())
        }
    }

    let min = bs.ns_iter_summ.min as u64;
    let avg = bs.ns_iter_summ.mean as u64;
    let max = bs.ns_iter_summ.max as u64;
    let cold = bs.ns_iter_summ.cold as u64;

    let mut min_fc = FmtCounter::new();
    if let Err(_) = core::write!(min_fc, "{}", min) {
        return;
    }
    let mut avg_fc = FmtCounter::new();
    if let Err(_) = core::write!(avg_fc, "{}", avg) {
        return;
    }
    let mut max_fc = FmtCounter::new();
    if let Err(_) = core::write!(max_fc, "{}", max) {
        return;
    }
    let mut cold_fc = FmtCounter::new();
    if let Err(_) = core::write!(cold_fc, "{}", cold) {
        return;
    }
    log::info!(
        "{:-<width$}",
        "-",
        width = min_fc.chars + avg_fc.chars + max_fc.chars + cold_fc.chars + 16
    );
    log::info!(
        "|Metric    |{:minw$}|{:avgw$}|{:maxw$}|{:coldw$}|",
        "Min",
        "Avg",
        "Max",
        "Cold",
        minw = min_fc.chars,
        avgw = avg_fc.chars,
        maxw = max_fc.chars,
        coldw = cold_fc.chars
    );
    log::info!(
        "{:-<width$}",
        "-",
        width = min_fc.chars + avg_fc.chars + max_fc.chars + cold_fc.chars + 16
    );
    log::info!("|time_nanos|{:3}|{:3}|{:3}|{:4}|", min, avg, max, cold);
    log::info!(
        "{:-<width$}",
        "-",
        width = min_fc.chars + avg_fc.chars + max_fc.chars + cold_fc.chars + 16
    );
}

#[cfg(feature = "machine_readable")]
fn print_samples(test: &TestDesc, bs: &bench::BenchSamples) {
    let min = bs.ns_iter_summ.min as u64;
    let avg = bs.ns_iter_summ.mean as u64;
    let max = bs.ns_iter_summ.max as u64;
    let cold = bs.ns_iter_summ.cold as u64;

    let (suite, bench) =
        test.name.as_slice().rsplit_once("::").unwrap_or((test.name.as_slice(), ""));
    log::info!("{{\"schema_version\": 3,");
    log::info!("\"suite_name\": \"{}\",", suite);
    log::info!("\"bench_name\": \"{}\",", bench);
    log::info!(
        "\"results\": \
        [{{\"metric_name\": \"time_nanos\", \
        \"min\": \"{}\", \
        \"max\": \"{}\", \
        \"avg\": \"{}\", \
        \"cold\": \"{}\", \
        \"raw_min\": {}, \
        \"raw_max\": {}, \
        \"raw_avg\": {}, \
        \"raw_cold\": {}}}",
        min,
        max,
        avg,
        cold,
        min,
        max,
        avg,
        cold,
    );
    log::info!("]}}");
}

impl Service for TestService {
    type Connection = ();
    type Message = ();

    fn on_connect(
        &self,
        _port: &PortCfg,
        handle: &Handle,
        _peer: &Uuid,
    ) -> tipc::Result<ConnectResult<Self::Connection>> {
        LOGGER.connect(handle)?;

        log::info!("[==========] Running {} tests from 1 test suite.\n", self.tests.len());

        let mut passed_tests = 0;
        let mut failed_tests = 0;
        let mut skipped_tests = 0;
        let mut total_ran = 0;
        let mut total_duration_ms = 0;
        for test in &self.tests {
            CONTEXT.reset();
            total_ran += 1;
            print_status(&test.desc, "RUN     ");
            let start_time_ns = get_time_ns();
            match test.testfn {
                StaticTestFn(f) => f(),
                StaticBenchFn(f) => {
                    let bs = bench::benchmark(|harness| f(harness));
                    print_samples(&test.desc, &bs);
                }
                _ => panic!("non-static tests passed to test::test_main_static"),
            }
            let duration_ms = (get_time_ns() - start_time_ns) / 1_000_000u64;
            total_duration_ms += duration_ms;
            if CONTEXT.skipped() {
                print_status_with_duration(&test.desc, " SKIPPED", duration_ms);
                skipped_tests += 1;
            } else if CONTEXT.all_ok() {
                print_status_with_duration(&test.desc, "      OK", duration_ms);
                passed_tests += 1;
            } else {
                print_status_with_duration(&test.desc, " FAILED ", duration_ms);
                failed_tests += 1;
            }
            if CONTEXT.hard_fail() {
                break;
            }
        }

        log::info!("[==========] {} tests ran ({} ms total).", total_ran, total_duration_ms);
        if passed_tests > 0 {
            log::info!("[  PASSED  ] {} tests.", passed_tests);
        }
        if skipped_tests > 0 {
            log::info!("[  SKIPPED ] {} tests.", skipped_tests);
        }
        if failed_tests > 0 {
            log::info!("[  FAILED  ] {} tests.", failed_tests);
        }

        let response = if failed_tests == 0 { TestMessage::Passed } else { TestMessage::Failed };
        handle.send(&response)?;

        LOGGER.disconnect();
        Ok(ConnectResult::CloseConnection)
    }

    fn on_message(
        &self,
        _connection: &Self::Connection,
        _handle: &Handle,
        _msg: Self::Message,
    ) -> tipc::Result<MessageResult> {
        Ok(MessageResult::CloseConnection)
    }

    fn on_disconnect(&self, _connection: &Self::Connection) {
        LOGGER.disconnect();
    }
}

/// A variant optimized for invocation with a static test vector.
/// This will panic (intentionally) when fed any dynamic tests.
///
/// This is the entry point for the main function generated by `rustc --test`
/// when panic=abort.
pub fn test_main_static_abort(tests: &[&TestDescAndFn]) {
    log::set_logger(&LOGGER).expect("Could not set global logger");
    log::set_max_level(log::LevelFilter::Info);

    let owned_tests: Vec<_> = tests.iter().map(make_owned_test).collect();

    // SAFETY: This static is declared in the crate being tested, so must be
    // external. This static should only ever be defined by the macro above.
    let port_str = unsafe { TEST_PORT };

    let cfg = PortCfg::new(port_str)
        .expect("Could not create port config")
        .allow_ta_connect()
        .allow_ns_connect();

    let test_service = TestService { tests: owned_tests };

    let buffer = [0u8; 4096];
    Manager::<_, _, 1, 4>::new(test_service, cfg, buffer)
        .expect("Could not create service manager")
        .run_event_loop()
        .expect("Test event loop failed");
}

/// Clones static values for putting into a dynamic vector, which test_main()
/// needs to hand out ownership of tests to parallel test runners.
///
/// This will panic when fed any dynamic tests, because they cannot be cloned.
fn make_owned_test(test: &&TestDescAndFn) -> TestDescAndFn {
    match test.testfn {
        StaticTestFn(f) => TestDescAndFn { testfn: StaticTestFn(f), desc: test.desc.clone() },
        StaticBenchFn(f) => TestDescAndFn { testfn: StaticBenchFn(f), desc: test.desc.clone() },
        _ => panic!("non-static tests passed to test::test_main_static"),
    }
}

/// Invoked when unit tests terminate. The normal Rust test harness supports
/// tests which return values, we don't, so we require the test to return unit.
pub fn assert_test_result(_result: ()) {}

/// Skip the current test case.
pub fn skip() {
    CONTEXT.skip();
}
