# Copyright (C) 2025 The Android Open Source Project
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

AUTHMGR_BE_AIDL_DIR = hardware/interfaces/security/see/authmgr/aidl

MODULE := $(LOCAL_DIR)

MODULE_CRATE_NAME := android_hardware_security_see_authmgr

MODULE_AIDL_LANGUAGE := rust

MODULE_AIDL_PACKAGE := android/hardware/security/see/authmgr

MODULE_AIDL_INCLUDES := \
	-I $(AUTHMGR_BE_AIDL_DIR) \

MODULE_AIDL_FLAGS := \
    --stability=vintf \

MODULE_AIDLS := \
    $(AUTHMGR_BE_AIDL_DIR)/$(MODULE_AIDL_PACKAGE)/DiceChainEntry.aidl \
    $(AUTHMGR_BE_AIDL_DIR)/$(MODULE_AIDL_PACKAGE)/DiceLeafArtifacts.aidl \
    $(AUTHMGR_BE_AIDL_DIR)/$(MODULE_AIDL_PACKAGE)/DicePolicy.aidl \
    $(AUTHMGR_BE_AIDL_DIR)/$(MODULE_AIDL_PACKAGE)/Error.aidl \
    $(AUTHMGR_BE_AIDL_DIR)/$(MODULE_AIDL_PACKAGE)/ExplicitKeyDiceCertChain.aidl \
    $(AUTHMGR_BE_AIDL_DIR)/$(MODULE_AIDL_PACKAGE)/IAuthMgrAuthorization.aidl \
    $(AUTHMGR_BE_AIDL_DIR)/$(MODULE_AIDL_PACKAGE)/SignedConnectionRequest.aidl \

include make/aidl.mk