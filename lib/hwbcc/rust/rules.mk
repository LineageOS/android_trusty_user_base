# Copyright (C) 2021 The Android Open Source Project
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#      http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#

LOCAL_DIR := $(GET_LOCAL_DIR)
LIBCPPBOR_DIR := $(if $(wildcard system/libcppbor),system/libcppbor,external/libcppbor)

MODULE := $(LOCAL_DIR)

MODULE_SRCS := $(LOCAL_DIR)/src/lib.rs

MODULE_CRATE_NAME := hwbcc

MODULE_SDK_LIB_NAME := hwbcc-rust

MODULE_LIBRARY_DEPS += \
	$(LIBCPPBOR_DIR) \
	$(call FIND_CRATE,coset) \
	$(call FIND_CRATE,log) \
	$(call FIND_CRATE,zerocopy) \
	$(call FIND_CRATE,num-derive) \
	$(call FIND_CRATE,num-traits) \
	external/boringssl \
	external/open-dice \
	packages/modules/Virtualization/libs/dice/open_dice \
	trusty/user/base/interface/hwbcc \
	trusty/user/base/lib/authgraph-rust/core \
	trusty/user/base/lib/authgraph-rust/boringssl \
	trusty/user/base/lib/hwbcc/common \
	trusty/user/base/lib/trusty-std \
	trusty/user/base/lib/tipc/rust \
	trusty/user/base/lib/system_state/rust \

MODULE_BINDGEN_ALLOW_TYPES := \
	hwbcc.* \

MODULE_BINDGEN_ALLOW_VARS := \
	HWBCC.* \
	DICE.* \
	ED25519.* \

MODULE_BINDGEN_FLAGS := \
	--with-derive-custom-struct=hwbcc.*=zerocopy::IntoBytes,zerocopy::FromBytes \

MODULE_BINDGEN_SRC_HEADER := $(LOCAL_DIR)/bindings.h

# Enable tests specific to the generic emulator build,
# which depend on the device-specific BCC key.
# Exclude trusty as a VM guest since on arm those builds run
# as part of the generic-arm64 platform but do not use the
# same hardcoded key.
ifeq (generic-arm64, $(PLATFORM))
ifeq (false,$(call TOBOOL,$(TRUSTY_VM_GUEST)))
MODULE_RUSTFLAGS += --cfg 'feature="generic-arm-unittest"'
endif
endif

# Some functionality of hwbcc does not exist in VMs
# like TEST_MODE and ns_deprivilege.
ifeq (true,$(call TOBOOL,$(TRUSTY_VM_GUEST)))
MODULE_RUSTFLAGS += --cfg 'feature="trusty_vm_guest"'
endif


MODULE_RUST_TESTS := true

# For test service
MANIFEST := $(LOCAL_DIR)/manifest.json

include make/library.mk
