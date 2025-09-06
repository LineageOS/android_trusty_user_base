/*
 * Copyright (C) 2021 The Android Open Source Project
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

//! Mapping from C types to their Rust equivalents

#![allow(non_camel_case_types)]

#[cfg(not(any(target_arch = "aarch64", target_arch = "arm", target_arch = "x86_64")))]
compile_error!("Only aarch64, arm, and x86_64 architectures are currently supported.");

pub use core::ffi::c_void;

#[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
pub type c_char = u8;

#[cfg(target_arch = "x86_64")]
pub type c_char = i8;

pub type c_schar = i8;

pub type c_uchar = u8;

pub type c_short = i16;

pub type c_ushort = u16;

pub type c_int = i32;
pub type c_uint = u32;

#[cfg(target_pointer_width = "32")]
pub type c_long = i32;
#[cfg(target_pointer_width = "64")]
pub type c_long = i64;

#[cfg(target_pointer_width = "32")]
pub type c_ulong = u32;
#[cfg(target_pointer_width = "64")]
pub type c_ulong = u64;

pub type c_longlong = i64;
pub type c_ulonglong = u64;

pub type c_uint8_t = u8;
pub type c_uint16_t = u16;
pub type c_uint32_t = u32;
pub type c_uint64_t = u64;

pub type c_int8_t = i8;
pub type c_int16_t = i16;
pub type c_int32_t = i32;
pub type c_int64_t = i64;
