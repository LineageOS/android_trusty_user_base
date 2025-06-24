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

use super::*;
use ::test::{assert, assert_eq, assert_ne, assert_ok, skip};
use authgraph_boringssl::BoringEcDsa;
use authgraph_core::key::{CertChain, EcVerifyKey};
use authgraph_core::traits::EcDsa;
use ciborium::Value;
use coset::CborSerializable;
use diced_open_dice::{bcc_handover_parse, DiceArtifacts as DicedDiceArtifacts};
use system_state::{SystemState, SystemStateFlag};

::test::init!();

const TEST_MAC_KEY: &'static [u8; HWBCC_MAC_KEY_SIZE as usize] = &[
    0xf4, 0xe2, 0xd2, 0xbb, 0x2d, 0x07, 0x16, 0xb9, 0x66, 0x4b, 0x73, 0xe8, 0x56, 0xd3, 0x6e, 0xfb,
    0x08, 0xb4, 0x01, 0xd8, 0x86, 0x38, 0xa7, 0x9a, 0x97, 0xb3, 0x98, 0x4f, 0x63, 0xdc, 0xef, 0xed,
];

const TEST_AAD: &'static [u8] = &[0xcf, 0xe1, 0x89, 0x39, 0xb1, 0x72, 0xbf, 0x4f, 0xa8, 0x0f];

const HWBCC_RUST_TEST_UUID: &str = "67925337-2c03-49ed-9240-d51b6fea3e30";

#[test]
fn test_get_emulator_dice_artifacts() {
    if !cfg!(feature = "generic-arm-unittest") {
        skip!("not running on emulator");
    }

    const EMULATOR_CDI_ATTEST: &'static [u8; DICE_CDI_SIZE as usize] = &[
        0x44, 0x26, 0x69, 0x94, 0x02, 0x34, 0x1c, 0xc8, 0x1d, 0x93, 0xc7, 0xb8, 0x47, 0xaf, 0x55,
        0xe8, 0xde, 0x8e, 0x79, 0x4c, 0x1b, 0x0f, 0xea, 0x99, 0x7f, 0x91, 0x83, 0x83, 0x7f, 0x26,
        0x7f, 0x93,
    ];

    const EMULATOR_CDI_SEAL: &'static [u8; DICE_CDI_SIZE as usize] = &[
        0xf7, 0xe5, 0xb0, 0x2b, 0xd0, 0xfa, 0x4d, 0x5b, 0xfa, 0xd8, 0x16, 0x24, 0xfa, 0xc8, 0x50,
        0xac, 0x4f, 0x1a, 0x3d, 0xb4, 0xbc, 0x02, 0xc9, 0xfd, 0xeb, 0xfe, 0x26, 0xfc, 0x28, 0x98,
        0x5b, 0xe8,
    ];

    let dice_artifacts_buf = &mut [0u8; HWBCC_MAX_RESP_PAYLOAD_LENGTH];

    let DiceArtifacts { artifacts } = assert_ok!(get_dice_artifacts(0, dice_artifacts_buf));
    let parsed_handover = assert_ok!(bcc_handover_parse(artifacts));

    // We don't expect the DICE chain on the emulator build.
    assert!(parsed_handover.bcc().is_none());

    let system_state_session =
        assert_ok!(SystemState::try_connect(), "could not connect to system state service");
    if system_state_session.get_flag(SystemStateFlag::AppLoadingUnlocked).unwrap() != 0 {
        assert_eq!(EMULATOR_CDI_ATTEST, parsed_handover.cdi_attest());
        assert_eq!(EMULATOR_CDI_SEAL, parsed_handover.cdi_seal());
    }
}

#[test]
fn test_ns_deprivilege() {
    if cfg!(feature = "trusty_vm_guest") {
        skip!("trusty vm guests do not support ns_deprivilege");
    }

    ns_deprivilege().expect("could not execute ns deprivilege");

    // ns_deprivilege should not block calls from secure world
    let dice_artifacts_buf = &mut [0u8; HWBCC_MAX_RESP_PAYLOAD_LENGTH];
    assert_ok!(get_dice_artifacts(0, dice_artifacts_buf));
}

#[test]
fn test_get_bcc_test_mode() {
    if cfg!(feature = "trusty_vm_guest") {
        skip!("trusty vm guests do not support HwBccMode::Test");
    }

    let mut bcc_buf = [0u8; HWBCC_MAX_RESP_PAYLOAD_LENGTH];

    // get a first set of keys
    let bcc = assert_ok!(get_bcc(HwBccMode::Test, &mut bcc_buf));
    let explicit_key_chain = assert_ok!(CertChain::from_non_explicit_key_cert_chain(bcc));
    let leaf_pub_key =
        assert_ok!(explicit_key_chain.validate(&BoringEcDsa), "Cert chain is not valid");
    let root_key = explicit_key_chain.root_key;

    // get second set of keys
    bcc_buf.fill(0);
    let bcc = assert_ok!(get_bcc(HwBccMode::Test, &mut bcc_buf));
    let explicit_key_chain2 = assert_ok!(CertChain::from_non_explicit_key_cert_chain(bcc));
    let leaf_pub_key2 =
        assert_ok!(explicit_key_chain2.validate(&BoringEcDsa), "Cert chain is not valid");
    let root_key2 = explicit_key_chain2.root_key;

    // the two sets of keys must be different in test mode
    assert_ne!(root_key, root_key2);
    assert_ne!(leaf_pub_key, leaf_pub_key2);
}

#[test]
fn test_get_emulator_bcc() {
    if !cfg!(feature = "generic-arm-unittest") {
        skip!("not running on emulator");
    }

    /*
     * Device key is hard-coded on emulator targets, i.e. BCC keys are fixed too.
     * We test that BCC keys don't change to make sure that we don't accidentally
     * change the key derivation procedure. Function of test TA app UUID.
     */
    const EMULATOR_PUB_KEY: &'static [u8; ED25519_PUBLIC_KEY_LEN as usize] = &[
        0xc3, 0xfc, 0x8c, 0x92, 0x1d, 0x52, 0xb2, 0x34, 0x9f, 0x6d, 0x59, 0xa3, 0xcd, 0xcd, 0x4a,
        0x8b, 0x1f, 0x97, 0xb6, 0x7b, 0xde, 0x2a, 0x7e, 0x2a, 0x46, 0xae, 0x98, 0x91, 0x47, 0xff,
        0x5a, 0xef,
    ];

    let mut emulator_pub_key = assert_ok!(
        EcVerifyKey::from_cose_key(
            coset::CoseKeyBuilder::new_okp_key()
                .param(
                    iana::OkpKeyParameter::Crv as i64,
                    Value::from(iana::EllipticCurve::Ed25519 as u64),
                )
                .param(iana::OkpKeyParameter::X as i64, Value::from(EMULATOR_PUB_KEY.as_slice()))
                .algorithm(coset::iana::Algorithm::EdDSA)
                .add_key_op(coset::iana::KeyOperation::Verify)
                .build()
        ),
        "Failed to create an EcVerifyKey from the pub key array"
    );
    emulator_pub_key.canonicalize_cose_key();

    let mut bcc_buf = [0u8; HWBCC_MAX_RESP_PAYLOAD_LENGTH];

    let bcc = assert_ok!(get_bcc(HwBccMode::Release, &mut bcc_buf));
    let explicit_key_cert_chain = assert_ok!(CertChain::from_non_explicit_key_cert_chain(bcc));
    let leaf_public_key =
        assert_ok!(explicit_key_cert_chain.validate(&BoringEcDsa), "Cert chain is not valid");

    // The emulator uses a hard-coded key and creates a degenerate cert, so we expect the
    // root key and the subject_public_key of the first cert to be the same.
    assert_eq!(emulator_pub_key, explicit_key_cert_chain.root_key);
    assert_eq!(explicit_key_cert_chain.root_key, leaf_public_key);
}

#[test]
fn test_sign_data_test_mode() {
    if cfg!(feature = "trusty_vm_guest") {
        skip!("trusty vm guests do not support HwBccMode::Test");
    }

    let mut cose_sign1_buf = [0u8; HWBCC_MAX_RESP_PAYLOAD_LENGTH];

    let sig = assert_ok!(sign_data(
        HwBccMode::Test,
        SigningAlgorithm::ED25519,
        TEST_MAC_KEY,
        TEST_AAD,
        &mut cose_sign1_buf,
    ));

    // Note that the best we can do here is assert that we have successfully
    // obtained a signature. This is because the Rust API does not support
    // hwbcc sessions; it creates a new session with every API call.
    // HwBccMode::Test will result in a different key if we try and call
    // get_bcc to retrieve the public key use to sign data.
    assert!(sig.len() > 0);
}

#[test]
fn test_sign_data() {
    let mut buff = [0u8; HWBCC_MAX_RESP_PAYLOAD_LENGTH];

    let sig_raw = assert_ok!(sign_data(
        HwBccMode::Release,
        SigningAlgorithm::ED25519,
        TEST_MAC_KEY,
        TEST_AAD,
        &mut buff,
    ));
    let sig = assert_ok!(coset::CoseSign1::from_slice(&sig_raw));

    // We validate that the data (TEST_MAC_KEY) was signed by the private
    // key associated with the public key of the leaf cert of the DICE chain.
    buff.fill(0);
    let bcc = assert_ok!(get_bcc(HwBccMode::Release, &mut buff));
    let explicit_key_cert_chain = assert_ok!(CertChain::from_non_explicit_key_cert_chain(bcc));
    let leaf_pub_key =
        assert_ok!(explicit_key_cert_chain.validate(&BoringEcDsa), "Cert chain is not valid");

    assert_ok!(sig.verify_signature(&TEST_AAD, |signature, data| {
        BoringEcDsa.verify_signature(&leaf_pub_key, data, signature)
    }));
}

#[test]
fn test_get_bcc() {
    let mut bcc_buf = [0u8; HWBCC_MAX_RESP_PAYLOAD_LENGTH];

    let bcc = assert_ok!(get_bcc(HwBccMode::Release, &mut bcc_buf));
    let explicit_key_chain = assert_ok!(CertChain::from_non_explicit_key_cert_chain(bcc));
    let _ = assert_ok!(explicit_key_chain.validate(&BoringEcDsa), "Cert chain is not valid");
}

/// When trusty is running within a VM, it uses PvmDice as its hwbcc server implementation.
/// PvmDice derives a leaf cert for each call to get_bcc that is specific to the calling TA.
/// It does so by including the UUID of the calling TA as the component_name of the cert.
/// This test checks that the component_name is correctly set when this test app calls get_bcc.
#[test]
fn test_vm_get_bcc_leaf_is_ta_specific() {
    if !cfg!(feature = "trusty_vm_guest") {
        skip!("not running in trusty vm guest");
    }

    let mut bcc_buf = [0u8; HWBCC_MAX_RESP_PAYLOAD_LENGTH];

    let bcc = assert_ok!(get_bcc(HwBccMode::Release, &mut bcc_buf));
    let explicit_key_cert_chain = assert_ok!(CertChain::from_non_explicit_key_cert_chain(bcc));
    let _ = assert_ok!(explicit_key_cert_chain.validate(&BoringEcDsa), "Cert chain is not valid");

    let leaf_cert_component_name =
        assert_ok!(explicit_key_cert_chain.get_leaf_cert_component_name());
    assert_eq!(Some(HWBCC_RUST_TEST_UUID.to_string()), leaf_cert_component_name);
}
