use alloc::collections::TryReserveError;
use alloc::ffi::CString;
use diced_open_dice::{
    bcc_handover_parse, retry_bcc_format_config_descriptor, retry_bcc_main_flow,
    retry_sign_cose_sign1_with_cdi_leaf_priv, Config, DiceArtifacts, DiceConfigValues, DiceMode,
    Hash, Hidden, InputValues, OwnedDiceArtifacts, CDI_SIZE, HASH_SIZE, HIDDEN_SIZE,
};
use hwbcc::srv::{HwBccOps, RequestContext};
use hwbcc::{HwBccError, HwBccMode};
use std::ffi::CStr;
use tipc::Uuid;
use trusty_sys::handle_t;

mod sys {
    include!(env!("BINDGEN_INC_FILE"));
}

// Currently we do not support dynamically loading apps into trusty pVMs.
// As such, all user space apps are packaged and signed together with
// the trusty kernel. This allows us to use empty authority and code
// hashes when deriving leaf certs, since all measurements were
// completed in 'pvmfw' before starting trusty and included in the leaf
// node of the bcc handover.
//
// These hashes must only be used for dice certs that measure non-loadable
// apps. If an app was dynamically loaded, it needs real code and authority
// measurements, which should likely be provided by the trusty apploader.
const TRUSTY_NON_LOADABLE_AUTHORITY_HASH: Hash = [0; HASH_SIZE];
const TRUSTY_NON_LOADABLE_CODE_HASH: Hash = [0; HASH_SIZE];

const EMPTY_HIDDEN_INPUTS: Hidden = [0; HIDDEN_SIZE];

const KEYMINT_UUID: Uuid =
    Uuid::new(0x5f902ace, 0x5e5c, 0x4cd8, [0xae, 0x54, 0x87, 0xb8, 0x8c, 0x22, 0xdd, 0xaf]);
const WIDEVINE_UUID: Uuid =
    Uuid::new(0x08d3ed40, 0xbde2, 0x448c, [0xa9, 0x1d, 0x75, 0xf1, 0x98, 0x9c, 0x57, 0xef]);

struct ComponentSpecificConfigValues {
    component_name: CString,
    rkp_vm_marker: bool,
}

#[derive(Debug)]
pub enum PvmDiceError {
    /// Errors returned from the diced_open_dice lib
    DiceError(diced_open_dice::DiceError),
    /// Used when trying to initialize PvmDice with a handover that
    /// does not include a cert chain.
    NoCertChainInHandover,
    /// Used if an allocation fails
    NoMemory,
    /// Returned on construction of PvmDice if we can't ensure that
    /// the apploader user space service is not and cannot run.
    ApploaderInvariantViolated,
}

impl From<diced_open_dice::DiceError> for PvmDiceError {
    fn from(e: diced_open_dice::DiceError) -> Self {
        PvmDiceError::DiceError(e)
    }
}

impl From<TryReserveError> for PvmDiceError {
    fn from(_: TryReserveError) -> Self {
        PvmDiceError::NoMemory
    }
}

/// An implementation of the hwbcc interface for trusty guests running in
/// protected virtual machines (pVMs).
pub struct PvmDice {
    bcc: Vec<u8>,
    cdi_attest: [u8; CDI_SIZE],
    cdi_seal: [u8; CDI_SIZE],
    // We keep this port open to maintain our invariant that
    // dynamic apps can't be loaded. See `enforce_no_apploader_invariant`.
    apploader_port_handle: handle_t,
}

impl Drop for PvmDice {
    fn drop(&mut self) {
        // SAFETY we invoke the close syscall with the expected type. The handle will
        // either be valid and closed, or close will safely return an error code.
        let rc = unsafe { trusty_sys::close(self.apploader_port_handle) };

        if rc != 0 {
            log::warn!("Failed to close apploader port handle when dropping PvmDice: {}", rc);
        }
    }
}

impl PvmDice {
    pub fn try_new(handover_buff: &[u8]) -> Result<Self, PvmDiceError> {
        let bcc_handover = bcc_handover_parse(handover_buff)?;

        if let Some(bcc) = bcc_handover.bcc() {
            Ok(PvmDice {
                bcc: bcc.to_vec(),
                cdi_attest: *bcc_handover.cdi_attest(),
                cdi_seal: *bcc_handover.cdi_seal(),
                apploader_port_handle: enforce_no_apploader_invariant()?,
            })
        } else {
            log::error!("attempting to initialize PvmDice without cert chain in handover");

            Err(PvmDiceError::DiceError(diced_open_dice::DiceError::DiceChainNotFound))
        }
    }

    fn derive_next_dice_artifacts(
        &self,
        request_ctx: &RequestContext,
    ) -> Result<OwnedDiceArtifacts, HwBccError> {
        let config_descriptor = config_descriptor_for_trusty_user_app(request_ctx)?;
        let input_values = InputValues::new(
            TRUSTY_NON_LOADABLE_CODE_HASH,
            Config::Descriptor(config_descriptor.as_slice()),
            TRUSTY_NON_LOADABLE_AUTHORITY_HASH,
            dice_mode_for_trusty_user_space_certs(),
            EMPTY_HIDDEN_INPUTS,
        );

        Ok(retry_bcc_main_flow(&self.cdi_attest, &self.cdi_seal, &self.bcc, &input_values)?)
    }
}

impl HwBccOps for PvmDice {
    fn init(&self, _: &RequestContext) -> Result<(), HwBccError> {
        Ok(())
    }

    fn close(&self, _: &RequestContext) {}

    fn get_bcc(
        &self,
        request_ctx: &RequestContext,
        mode: HwBccMode,
    ) -> Result<Vec<u8>, HwBccError> {
        let _ = check_not_test_mode(mode)?;

        let next_dice_artifacts = self.derive_next_dice_artifacts(request_ctx)?;

        let mut bcc = Vec::new();

        // We should always have `bcc` if retry_bcc_main_flow succeeds, but best to try and fail
        // gracefully.
        let next_bcc = next_dice_artifacts.bcc().ok_or(trusty_sys::Error::NotValid)?;
        bcc.try_reserve(next_bcc.len())?;
        bcc.extend_from_slice(&next_bcc);

        Ok(bcc)
    }

    fn sign_data<'a>(
        &self,
        request_ctx: &RequestContext,
        data: &'a [u8],
        aad: &'a [u8],
        mode: HwBccMode,
    ) -> Result<Vec<u8>, HwBccError> {
        let _ = check_not_test_mode(mode)?;

        // TODO: b/404559104 - Keep per-session state around, to avoid re-deriving artifacts
        // for the common case of calling `get_bcc` and then `sign_data`. Doing so now would
        // have no impact because the rust hwbcc client opens a new connection for each operation.
        let dice_artifacts = self.derive_next_dice_artifacts(request_ctx)?;

        Ok(retry_sign_cose_sign1_with_cdi_leaf_priv(data, aad, &dice_artifacts)?)
    }
}

/// A helper to check for HwBcc::TestMode and return an error if it's used.
/// This mode was specific to V1 of the Keymint HAL and is not relevant
/// for VM use cases where we expect callers are using later versions of
/// the RKP protocol.
fn check_not_test_mode(mode: HwBccMode) -> Result<(), HwBccError> {
    if mode == HwBccMode::Test {
        log::error!("pvmdice does not support HwBccMode::TestMode");
        return Err(HwBccError::NotAllowed);
    }

    Ok(())
}

/// Get the DICE mode to be used for trusty user space leaf certs
fn dice_mode_for_trusty_user_space_certs() -> DiceMode {
    if cfg!(TEST_BUILD) {
        return DiceMode::kDiceModeDebug;
    }

    DiceMode::kDiceModeNormal
}

/// Generate config descriptor for a dice derivation based on the calling app.
fn config_descriptor_for_trusty_user_app(
    request_ctx: &RequestContext,
) -> Result<Vec<u8>, HwBccError> {
    let component_config = dice_component_specific_config_values(request_ctx);

    let config_values = DiceConfigValues {
        component_name: Some(&component_config.component_name),
        component_version: Some(1),
        resettable: false,
        rkp_vm_marker: component_config.rkp_vm_marker,
        // We expect to rely on the security_version of the dice node representing the
        // entire OS (our parent node) for non-loadable apps.
        security_version: Some(1),
    };

    Ok(retry_bcc_format_config_descriptor(&config_values)?)
}

/// Derive a component name from an incoming request.
/// There are certain UUIDs that have special treatment so that they
/// can be recognized by the RKP server. For all other UUIDs, we use
/// a stringified app UUID.
fn dice_component_specific_config_values(
    request_ctx: &RequestContext,
) -> ComponentSpecificConfigValues {
    match request_ctx.peer {
        KEYMINT_UUID => {
            ComponentSpecificConfigValues { component_name: c"keymint".into(), rkp_vm_marker: true }
        }
        WIDEVINE_UUID => ComponentSpecificConfigValues {
            component_name: c"widevine".into(),
            rkp_vm_marker: true,
        },
        _ => ComponentSpecificConfigValues {
            component_name: CString::new(request_ctx.peer.to_string()).unwrap(),
            rkp_vm_marker: false,
        },
    }
}

/// A helper that ensures that the trusty user space apploader does not run
/// in the same OS as pvmdice. We only emit a warning when TEST_BUILD is set and apploader
/// exists because in that case, the dice node will be marked as Debug, so verifiers
/// of the dice chain should recognize the app as different from the production
/// variant.
fn enforce_no_apploader_invariant() -> Result<handle_t, PvmDiceError> {
    let port = CStr::from_bytes_with_nul(sys::APPLOADER_PORT)
        .map_err(|_| PvmDiceError::ApploaderInvariantViolated)?;

    // SAFETY: This is a syscall. `port` is a valid CStr (checked above) and is only
    // read by port_create.
    let rc = unsafe {
        trusty_sys::port_create(
            port.as_ptr(),
            1, /*num_recv_bufs */
            1, /*recv_buf_size */
            0, /*flags */
        )
    };

    if rc < 0 {
        if cfg!(TEST_BUILD) {
            log::warn!("Failed to claim apploader port. Ignoring because TEST_BUILD is set.");
            return Ok(-1);
        }

        log::error!("Failed to claim apploader port. PvmDice invariant check failed: {}", rc);

        return Err(PvmDiceError::ApploaderInvariantViolated);
    }

    Ok(rc.try_into().map_err(|_| {
        log::error!("Invalid handle for apploader port {}. PvmDice invariant check failed.", rc);

        PvmDiceError::ApploaderInvariantViolated
    })?)
}

#[cfg(test)]
mod test {
    use crate::PvmDice;
    use authgraph_boringssl::BoringEcDsa;
    use authgraph_core::key::DiceChainEntry;
    use authgraph_core::traits::EcDsa;
    use ciborium::value::Value;
    use coset::{AsCborValue, CborSerializable};
    use diced_open_dice::DiceArtifacts;
    use diced_sample_inputs::make_sample_bcc_and_cdis;
    use hwbcc::srv::{HwBccOps, RequestContext};
    use hwbcc::HwBccMode;
    use test::expect;
    use tipc::Uuid;
    ::test::init!();

    const TEST_MESSAGE: &[u8] = "pvmdice test message".as_bytes();
    const TEST_AAD: &[u8] = "pvmdice test aad".as_bytes();

    #[test]
    fn test_init_empty_handover() {
        let handover: &[u8] = &[];
        let pvmdice = PvmDice::try_new(handover);
        expect!(pvmdice.is_err())
    }

    #[test]
    fn test_init_bad_handover() {
        let handover: &[u8] = &[0x12, 0x34, 0x56, 0x78];
        let pvmdice = PvmDice::try_new(handover);
        expect!(pvmdice.is_err())
    }

    #[test]
    fn test_get_bcc() {
        let dice_artifacts = make_sample_bcc_and_cdis().unwrap();
        let handover = to_bcc_handover(&dice_artifacts);
        let pvmdice = PvmDice::try_new(&handover).unwrap();

        let rq = RequestContext {
            peer: Uuid::new_from_string("c07129be-cabb-4d4d-837f-ea8fd204dcf1").unwrap(),
        };

        // TODO: b/393356669 - validate the cert chain.
        let _ = pvmdice.get_bcc(&rq, HwBccMode::Release).unwrap();
    }

    #[test]
    fn verify_sign_data_with_leaf_pub_key() {
        let dice_artifacts = make_sample_bcc_and_cdis().unwrap();
        let handover = to_bcc_handover(&dice_artifacts);
        let pvmdice = PvmDice::try_new(&handover).unwrap();

        let rq = RequestContext {
            peer: Uuid::new_from_string("c07129be-cabb-4d4d-837f-ea8fd204dcf1").unwrap(),
        };

        let signed_payload_res =
            pvmdice.sign_data(&rq, &TEST_MESSAGE, &TEST_AAD, HwBccMode::Release);
        expect!(signed_payload_res.is_ok());
        let signed_payload = signed_payload_res.unwrap();
        let signed_payload_cose = coset::CoseSign1::from_slice(&signed_payload).unwrap();

        // Now verify with the signature in the leaf cert from a chain
        // retrieved from get_bcc.
        let ecdsa = BoringEcDsa;
        let non_explicit_chain_res = pvmdice.get_bcc(&rq, HwBccMode::Release);
        expect!(non_explicit_chain_res.is_ok());
        let non_explicit_chain = non_explicit_chain_res.unwrap();
        let leaf_cert = leaf_cert_from_non_explicit_chain(&non_explicit_chain);
        let leaf_cert_pub_key = leaf_cert.payload.subject_pub_key.unwrap();

        expect!(signed_payload_cose
            .verify_signature(&TEST_AAD, |sign, data| ecdsa.verify_signature(
                &leaf_cert_pub_key,
                data,
                sign
            ))
            .is_ok());
    }

    fn leaf_cert_from_non_explicit_chain(dice_chain: &Vec<u8>) -> DiceChainEntry {
        let mut chain_value = Value::from_slice(dice_chain.as_slice()).expect("invalid cbor");
        let chain_arr: &mut Vec<Value> =
            chain_value.as_array_mut().expect("get_bcc should return a CBOR array");
        let last_cert_cbor = chain_arr.remove(chain_arr.len() - 1);

        DiceChainEntry::from_cbor_value(last_cert_cbor).expect("Invalid leaf cert")
    }

    fn to_bcc_handover(dice_artifacts: &dyn DiceArtifacts) -> Vec<u8> {
        let dice_chain: Value = ciborium::from_reader(&mut dice_artifacts.bcc().unwrap()).unwrap();
        let bcc_handover = Value::Map(vec![
            (Value::Integer(1.into()), Value::Bytes(dice_artifacts.cdi_attest().to_vec())),
            (Value::Integer(2.into()), Value::Bytes(dice_artifacts.cdi_seal().to_vec())),
            (Value::Integer(3.into()), dice_chain),
        ]);
        let mut data = Vec::new();
        ciborium::into_writer(&bcc_handover, &mut data)
            .expect("serialization of bcc_handover failed");
        data
    }
}
