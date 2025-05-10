# Copyright (C) 2022 The Android Open Source Project
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

# Generate Rust bindings for C headers
#
# Bindgen reads C headers and generates compatible Rust declarations of types
# and APIs. This allows us to keep Rust code in sync with C code that it depends
# on.
#
# Input variables:
# MODULE_BINDGEN_ALLOW_FILES
# MODULE_BINDGEN_ALLOW_FUNCTIONS
# MODULE_BINDGEN_ALLOW_TYPES
# MODULE_BINDGEN_ALLOW_VARS
# MODULE_BINDGEN_BLOCK_TYPES
# MODULE_BINDGEN_CTYPES_PREFIX
# MODULE_BINDGEN_FLAGS
# MODULE_BINDGEN_OUTPUT_ENV_VAR
# MODULE_BINDGEN_SRC_HEADER
# MODULE_BINDGEN_OUTPUT_FILE_NAME

ifeq ($(strip $(MODULE_BINDGEN_SRC_HEADER)),)
$(error $(MODULE): MODULE_BINDGEN_SRC_HEADER is required to use bindgen.mk)
endif

BINDGEN := $(CLANG_TOOLS_BINDIR)/bindgen

ifeq ($(strip $(MODULE_BINDGEN_OUTPUT_FILE_NAME)),)
MODULE_BINDGEN_OUTPUT_FILE := $(call TOBUILDDIR,$(patsubst %.h,%.rs,$(MODULE_BINDGEN_SRC_HEADER)))
else
MODULE_BINDGEN_OUTPUT_FILE := $(call TOBUILDDIR,$(dir $(MODULE_BINDGEN_SRC_HEADER))$(MODULE_BINDGEN_OUTPUT_FILE_NAME).rs)
endif

# Trusty rust is all no_std
ifeq ($(MODULE_IS_KERNEL),true)
MODULE_BINDGEN_FLAGS += --use-core --ctypes-prefix 'core::ffi'
else
MODULE_BINDGEN_FLAGS += --use-core --ctypes-prefix 'trusty_sys'
endif

ifneq ($(strip $(MODULE_BINDGEN_CTYPES_PREFIX)),)
MODULE_BINDGEN_FLAGS += --ctypes-prefix $(MODULE_BINDGEN_CTYPES_PREFIX)
endif

MODULE_BINDGEN_FLAGS += $(addprefix --allowlist-var ,$(MODULE_BINDGEN_ALLOW_VARS))
MODULE_BINDGEN_FLAGS += $(addprefix --allowlist-type ,$(MODULE_BINDGEN_ALLOW_TYPES))
MODULE_BINDGEN_FLAGS += $(addprefix --blocklist-type ,$(MODULE_BINDGEN_BLOCK_TYPES))
MODULE_BINDGEN_FLAGS += $(addprefix --allowlist-function ,$(MODULE_BINDGEN_ALLOW_FUNCTIONS))
MODULE_BINDGEN_FLAGS += $(addprefix --allowlist-file ,$(MODULE_BINDGEN_ALLOW_FILES))
# other sanitizer flags if present will cause bindgen to fail unless we pass
# -fno-sanitize-ignorelist, see https://issues.chromium.org/issues/40265121
BINDGEN_MODULE_COMPILEFLAGS := $(MODULE_COMPILEFLAGS) -fvisibility=default -fno-sanitize-ignorelist

ifeq ($(call TOBOOL,$(TRUSTY_USERSPACE)),true)
# library.mk overrides GLOBAL_COMPILEFLAGS and GLOBAL_INCLUDES for the .o, so we
# must do the same here for bindgen
$(MODULE_BINDGEN_OUTPUT_FILE): GLOBAL_COMPILEFLAGS := $(GLOBAL_SHARED_COMPILEFLAGS) $(GLOBAL_USER_COMPILEFLAGS) $(GLOBAL_USER_IN_TREE_COMPILEFLAGS) $(GLOBAL_USER_IN_TREE_CPPFLAGS)
$(MODULE_BINDGEN_OUTPUT_FILE): GLOBAL_INCLUDES := $(addprefix -I,$(GLOBAL_UAPI_INCLUDES) $(GLOBAL_SHARED_INCLUDES) $(GLOBAL_USER_INCLUDES))
endif

-include $(MODULE_BINDGEN_OUTPUT_FILE).d

MODULE_BINDGEN_DEFINES := $(MODULE_DEFINES)
MODULE_BINDGEN_DEFINES += MODULE_BINDGEN_FLAGS=\"$(call clean_defines,$(MODULE_BINDGEN_FLAGS))\"
MODULE_BINDGEN_DEFINES += BINDGEN_MODULE_COMPILEFLAGS=\"$(call clean_defines,$(BINDGEN_MODULE_COMPILEFLAGS))\"
MODULE_BINDGEN_DEFINES += BINDGEN_MODULE_INCLUDES=\"$(call clean_defines,$(BINDGEN_MODULE_INCLUDES))\"

MODULE_BINDGEN_CONFIG := $(call TOBUILDDIR,$(dir $(MODULE_BINDGEN_SRC_HEADER))/module_bindgen_config.h)
$(MODULE_BINDGEN_CONFIG): MODULE_BINDGEN_DEFINES:=$(MODULE_BINDGEN_DEFINES)
$(MODULE_BINDGEN_CONFIG): MODULE:=$(MODULE)
$(MODULE_BINDGEN_CONFIG): configheader
	@$(call INFO_DONE,$(MODULE),generating bindgen config header, $@)
	@$(call MAKECONFIGHEADER,$@,MODULE_BINDGEN_DEFINES)

$(MODULE_BINDGEN_OUTPUT_FILE): BINDGEN := $(BINDGEN)
$(MODULE_BINDGEN_OUTPUT_FILE): BINDGEN_MODULE_COMPILEFLAGS := $(BINDGEN_MODULE_COMPILEFLAGS)
$(MODULE_BINDGEN_OUTPUT_FILE): BINDGEN_MODULE_INCLUDES := $(addprefix -I,$(MODULE_INCLUDES))
$(MODULE_BINDGEN_OUTPUT_FILE): ARCH_COMPILEFLAGS := $(ARCH_$(ARCH)_COMPILEFLAGS)
$(MODULE_BINDGEN_OUTPUT_FILE): DEFINES := $(addprefix -D,$(MODULE_DEFINES))
$(MODULE_BINDGEN_OUTPUT_FILE): MODULE_BINDGEN_FLAGS := $(MODULE_BINDGEN_FLAGS)
$(MODULE_BINDGEN_OUTPUT_FILE): RUSTFMT_PATH := $(RUST_BINDIR)/rustfmt
$(MODULE_BINDGEN_OUTPUT_FILE): $(MODULE_BINDGEN_SRC_HEADER) $(BINDGEN) $(MODULE_SRCDEPS) $(CONFIGHEADER) $(MODULE_BINDGEN_CONFIG)
	@$(MKDIR)
	$(NOECHO)
	CLANG_PATH=$(BINDGEN_CLANG_PATH) \
	LIBCLANG_PATH=$(BINDGEN_LIBCLANG_PATH) \
	RUSTFMT=$(RUSTFMT_PATH) \
	$(BINDGEN) $< -o $@ $(MODULE_BINDGEN_FLAGS) --depfile $@.d -- $(GLOBAL_COMPILEFLAGS) $(ARCH_COMPILEFLAGS) $(BINDGEN_MODULE_COMPILEFLAGS) $(BINDGEN_MODULE_INCLUDES) $(GLOBAL_INCLUDES) $(DEFINES) \
	|| ( \
	    err=$$?; \
	    echo "Bindgen failed with exit code $$err" >&2; \
	    rm -f $@; \
	    exit $$err \
	   )

MODULE_SRCDEPS += $(MODULE_BINDGEN_OUTPUT_FILE)

ifeq ($(MODULE_BINDGEN_OUTPUT_ENV_VAR),)
MODULE_BINDGEN_OUTPUT_ENV_VAR := BINDGEN_INC_FILE
endif

# MODULE_BINDGEN_OUTPUT_ENV_VAR is not compatible with Soong as Soong does not allow
# custom `envflags`. bindgen modules should resort to using `MODULE_BINDGEN_OUTPUT_FILE`
# and `include!(concat!(env!("OUT_DIR"), "/{MODULE_BINDGEN_OUTPUT_FILE}"))` to include
# the generated bindgen file.
MODULE_RUST_ENV := $(MODULE_RUST_ENV) $(MODULE_BINDGEN_OUTPUT_ENV_VAR)=$(MODULE_BINDGEN_OUTPUT_FILE) OUT_DIR=$(dir $(MODULE_BINDGEN_OUTPUT_FILE))

$(info $(MODULE_BINDGEN_OUTPUT_FILE))

MODULE_BINDGEN_ALLOW_FILES :=
MODULE_BINDGEN_ALLOW_FUNCTIONS :=
MODULE_BINDGEN_ALLOW_TYPES :=
MODULE_BINDGEN_ALLOW_VARS :=
MODULE_BINDGEN_BLOCK_TYPES :=
MODULE_BINDGEN_CONFIG :=
MODULE_BINDGEN_CTYPES_PREFIX :=
MODULE_BINDGEN_DEFINES :=
MODULE_BINDGEN_OUTPUT_ENV_VAR :=
MODULE_BINDGEN_OUTPUT_FILE_NAME :=
MODULE_BINDGEN_SRC_HEADER :=

BINDGEN :=
MODULE_BINDGEN_FLAGS :=
BINDGEN_MODULE_COMPILEFLAGS :=
MODULE_BINDGEN_OUTPUT_FILE :=
