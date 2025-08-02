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

MODULE := $(LOCAL_DIR)

HASHBROWN_DIR = $(RUST_BINDIR)/../lib/rustlib/src/rust/vendor/hashbrown

MODULE_SRCS := $(HASHBROWN_DIR)/src/lib.rs

MODULE_CRATE_NAME := hashbrown

MODULE_RUST_EDITION := 2021

MODULE_ADD_IMPLICIT_DEPS := false

MODULE_RUSTFLAGS += \
	--cfg 'feature="rustc-dep-of-std"' \
	--cfg 'feature="nightly"' \
	--cfg 'feature="rustc-internal-api"' \
	--cfg 'feature="raw-entry"' \

# Needed to prevent "`hashbrown::HashSet::<T, S>::with_hasher` cannot be (indirectly) exposed to stable"
# error when compiling against internal core/std libraries.
MODULE_RUSTFLAGS += \
	-Z force-unstable-if-unmarked \

# Suppress warnings since the crate generates warnings when being built as a
# dependency of std. We use `--cap-lints=allow` because that's done when
# building dependencies for rustc.
MODULE_RUSTFLAGS += \
	--cap-lints=allow \

MODULE_LIBRARY_DEPS += \
	trusty/user/base/lib/libcore-rust \
	trusty/user/base/lib/liballoc-rust \
	trusty/user/base/lib/libcompiler_builtins-rust \

# TODO: figure out why as of Rust 1.82, rustdoc fails nsjailed Soong builds
# whereas regular (build.py) builds still work.
MODULE_SKIP_DOCS := true

include make/library.mk
