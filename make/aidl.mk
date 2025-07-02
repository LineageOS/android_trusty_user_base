# Copyright (c) 2022, Google, Inc. All rights reserved
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

# Build an AIDL module for Trusty
#
# args:
# MODULE : module name (required)
# MODULE_AIDLS: list of AIDL files
# MODULE_AIDL_FLAGS: optional flags for the AIDL_TOOL binary
# MODULE_AIDL_PACKAGE: a path that matches the directory structure of the AIDL
#     package utilized in the module. For example, declaring
#     `package com.android.trusty.binder` should correspond to a
#     MODULE_AIDL_PACKAGE of com/android/trusty/binder. If MODULE_AIDL_PACKAGE
#     is not defined, it implies that there is no top-level `package`
#     declaration.
# MODULE_AIDL_LANGUAGE: the language to auto-generate the files for. Current
#     options are `cpp` and `rust`.
# MODULE_AIDL_RUST_DEPS: names of Rust AIDL crates that the current module
#     depends on. Until we find a way to automatically infer these, users
#     will have to specify them manually.
# MODULE_AIDL_STABLE: whether this is a stable versioned interface. Will set a
#     version (specified by MODULE_AIDL_VERSION below) and a hash read from the
#     .hash file (defaults to "notfrozen").
# MODULE_AIDL_VERSION: the version of the interface, if MODULE_AIDL_STABLE is
#     true. Defaults to 1.

# Check that there are is at most one package specified
ifeq ($(filter $(words $(MODULE_AIDL_PACKAGE)),0 1),)
$(error $(MODULE) has the following packages $(MODULE_AIDL_PACKAGE), but only one is supported)
endif

ifeq ($(MODULE_AIDL_LANGUAGE),)
MODULE_AIDL_LANGUAGE := cpp
endif

ifeq ($(MODULE_AIDL_LANGUAGE),cpp)
AIDL_EXT := cpp
AIDL_HEADER_DIR := $(BUILDDIR)/include
else ifeq ($(MODULE_AIDL_LANGUAGE),rust)
AIDL_EXT := rs
AIDL_HEADER_DIR :=
else
$(error "Unsupported AIDL language: $(MODULE_AIDL_LANGUAGE)")
endif

# TODO: this implies all sources are in MODULE_AIDL_PACKAGE or are subpackages
# of MODULE_AIDL_PACKAGE; support multiple packages
GET_AIDL_PACKAGE_ROOT = $(if $(MODULE_AIDL_PACKAGE),$(firstword $(subst $(MODULE_AIDL_PACKAGE), ,$1)),$(dir $1))

ifneq (,$(ANDROID_BUILD_TOP))
# Use the aidl tool from the build output if it exists
AIDL_TOOL ?= $(wildcard $(ANDROID_BUILD_TOP)/out/host/linux-x86/bin/aidl)
else
AIDL_TOOL ?= $(wildcard prebuilts/build-tools/linux-x86/bin/aidl)
endif

ifeq ($(AIDL_TOOL),)
$(error No AIDL_TOOL. Please build the AIDL compiler or checkout the main-trusty branch)
endif

MODULE_AIDL_INCLUDES ?=
AIDL_SRCS := $(call TOBUILDDIR,$(patsubst %.aidl,%.$(AIDL_EXT),$(MODULE_AIDLS)))
AIDL_RUST_GLUE_TOOL ?= system/tools/aidl/build/aidl_rust_glue.py
MODULE_AIDL_INCLUDES += $(foreach dir,$(sort $(foreach src,$(MODULE_AIDLS),$(call GET_AIDL_PACKAGE_ROOT,$(src)))), -I $(patsubst %/,%,$(dir)))

ifeq ($(call TOBOOL,$(MODULE_AIDL_STABLE)),true)
MODULE_AIDL_VERSION ?= 1
MODULE_AIDL_FLAGS += --version $(MODULE_AIDL_VERSION)

# List of all the .hash files for this module, so we can trigger a rebuild
# whenever one of them changes. For now we add all of them as dependencies to
# all the .aidl files in order to simplify our implementation.
# TODO: Ideally we would add each .hash file as a dependency only for the .aidl
# files that actually require it, but that would complicate the implementation.
AIDL_HASHES := $(wildcard $(foreach src,$(MODULE_AIDLS),$(call GET_AIDL_PACKAGE_ROOT,$(src))/.hash))
endif

# This macro covers three cases, matching what Soong does:
# * The interface is unstable
# 	No --hash flag is passed in at all
# * The interface is stable but no .hash file
# 	We use "notfrozen" as the hash
# * The interface is stable with a .hash
# 	Pass in --hash with the last line of the file as the hash value
GET_AIDL_HASH_FLAG = $(if $(filter-out 0 false,$(MODULE_AIDL_STABLE)),\
	--hash $(or \
		$(foreach hf,$(wildcard $(call GET_AIDL_PACKAGE_ROOT,$1)/.hash),$(shell tail -1 $(hf))),\
		notfrozen),)

# TODO: support multiple, disparate packages; for AIDL interfaces with package paths,
# the output directory for the tool should be at the root of
# the package path. The compiler creates one subdirectory
# per package component, e.g., com.foo.IFoo goes into com/foo/IFoo.cpp.
# Luckily the .aidl files are also required to follow this structure,
# so the input file is also com/foo/IFoo.aidl.
$(AIDL_SRCS): AIDL_HASHES := $(AIDL_HASHES)
$(AIDL_SRCS): AIDL_HEADER_DIR := $(AIDL_HEADER_DIR)
$(AIDL_SRCS): AIDL_EXT := $(AIDL_EXT)
$(AIDL_SRCS): AIDL_TOOL := $(AIDL_TOOL)
$(AIDL_SRCS): MODULE_AIDL_INCLUDES := $(MODULE_AIDL_INCLUDES)
$(AIDL_SRCS): MODULE_AIDL_FLAGS := $(MODULE_AIDL_FLAGS)
$(AIDL_SRCS): MODULE_AIDL_LANGUAGE := $(MODULE_AIDL_LANGUAGE)
$(AIDL_SRCS): MODULE_AIDL_PACKAGE := $(MODULE_AIDL_PACKAGE)
$(AIDL_SRCS): MODULE_AIDL_STABLE := $(MODULE_AIDL_STABLE)
$(AIDL_SRCS): MODULE := $(MODULE)
$(AIDL_SRCS): $(BUILDDIR)/%.$(AIDL_EXT): %.aidl $(AIDL_TOOL) $(AIDL_HASHES)
	@$(MKDIR)
	@if [ -n "$(AIDL_HEADER_DIR)" ]; then mkdir -p $(AIDL_HEADER_DIR); fi
	@$(call ECHO,$(MODULE),generating from AIDL,$@)
	$(NOECHO)$(AIDL_TOOL) --lang=$(MODULE_AIDL_LANGUAGE) --structured $(MODULE_AIDL_INCLUDES) \
		$(call GET_AIDL_HASH_FLAG,$<) \
		$(foreach dir,$(AIDL_HEADER_DIR),-h $(dir)) -o $(call GET_AIDL_PACKAGE_ROOT,$@) $(MODULE_AIDL_FLAGS) $<
	@$(call ECHO_DONE_SILENT,$(MODULE),generating from AIDL,$@)

ifeq ($(MODULE_AIDL_LANGUAGE),cpp)
MODULE_SRCS += $(AIDL_SRCS)

# AIDL generates .cpp files which depend on the binder and C++ modules
ifeq ($(call TOBOOL,$(TRUSTY_NEW_MODULE_SYSTEM)),false)
MODULE_DEPS += \
	trusty/kernel/lib/libcxx-trusty \
	frameworks/native/libs/binder/trusty/kernel
else
MODULE_LIBRARY_DEPS += \
	trusty/user/base/lib/libstdc++-trusty \
	frameworks/native/libs/binder/trusty
endif

MODULE_EXPORT_INCLUDES += $(AIDL_HEADER_DIR)

# Ensure that all auto-generated code, including headers, is
# emitted before downstream dependencies
MODULE_EXPORT_SRCDEPS += $(AIDL_SRCS)
else # Rust
AIDL_ROOT_RS := $(sort $(foreach src,$(AIDL_SRCS),$(call GET_AIDL_PACKAGE_ROOT,$(src))/$(MODULE_CRATE_NAME).rs))

ifneq ($(words $(AIDL_ROOT_RS)),1)
$(error Unable to determine root AIDL .rs file for $(MODULE))
endif

# Generate the top-level aidl_lib.rs for this module
$(AIDL_ROOT_RS): AIDL_RUST_GLUE_TOOL := $(AIDL_RUST_GLUE_TOOL)
$(AIDL_ROOT_RS): AIDL_SRCS := $(AIDL_SRCS)
$(AIDL_ROOT_RS): MODULE_AIDL_RUST_DEPS := $(foreach crate,$(MODULE_AIDL_RUST_DEPS),-I $(crate))
$(AIDL_ROOT_RS): $(AIDL_SRCS) $(AIDL_RUST_GLUE_TOOL)
	@echo generating $@ from AIDL Rust glue
	$(NOECHO)$(AIDL_RUST_GLUE_TOOL) $(MODULE_AIDL_RUST_DEPS) $@ $(dir $@) $(AIDL_SRCS)

MODULE_LIBRARY_DEPS += \
	frameworks/native/libs/binder/trusty/rust \
	$(call FIND_CRATE,async-trait) \
	$(call FIND_CRATE,lazy_static) \

# Disable all lints for auto-generated Rust sources
MODULE_RUSTFLAGS += --cap-lints allow

MODULE_SRCS += $(AIDL_ROOT_RS)
MODULE_EXPORT_SRCDEPS += $(AIDL_ROOT_RS)
endif

# Build the AIDL module into a library
include make/library.mk

MODULE_AIDLS :=
MODULE_AIDL_INCLUDES :=
MODULE_AIDL_FLAGS :=
MODULE_AIDL_PACKAGE :=
MODULE_AIDL_LANGUAGE :=
MODULE_AIDL_RUST_DEPS :=
MODULE_AIDL_STABLE :=
MODULE_AIDL_VERSION :=
AIDL_EXT :=
AIDL_HASHES :=
AIDL_HEADER_DIR :=
AIDL_SRCS :=
AIDL_ROOT_RS :=
