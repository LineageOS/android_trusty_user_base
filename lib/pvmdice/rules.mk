# Copyright (C) 2024 The Android Open Source Project
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

MODULE := $(LOCAL_DIR)

MODULE_SRCS := $(LOCAL_DIR)/src/lib.rs

MODULE_CRATE_NAME := pvmdice

MODULE_SDK_LIB_NAME := pvmdice-rust

MODULE_LIBRARY_DEPS += \
	$(call FIND_CRATE,log) \
	$(call FIND_CRATE,coset) \
	$(call FIND_CRATE,ciborium) \
	trusty/user/base/interface/apploader \
	trusty/user/base/lib/authgraph-rust/core \
	trusty/user/base/lib/authgraph-rust/boringssl \
	trusty/user/base/lib/hwbcc/rust \
	trusty/user/base/lib/tipc/rust \
	trusty/user/base/lib/trusty-std \
	packages/modules/Virtualization/libs/dice/open_dice \
	packages/modules/Virtualization/libs/dice/sample_inputs \

MODULE_BINDGEN_ALLOW_FUNCTIONS :=
MODULE_BINDGEN_ALLOW_TYPES :=
MODULE_BINDGEN_ALLOW_VARS := \
	APPLOADER_PORT \

MODULE_BINDGEN_SRC_HEADER := $(LOCAL_DIR)/bindings.h

MODULE_RUST_TESTS := true

# For test service
MANIFEST := $(LOCAL_DIR)/manifest.json

include make/library.mk
