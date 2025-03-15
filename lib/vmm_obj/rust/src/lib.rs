/*
 * Copyright (C) 2024 The Android Open Source Project
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

//! # Interface library for communicating with the vmm obj service.

#![no_std]

use core::ffi::CStr;
use trusty_std::vec::Vec;
use trusty_sys::c_void;

#[allow(non_upper_case_globals)]
#[allow(non_camel_case_types)]
#[allow(unused)]
pub mod sys {
    include!(env!("BINDGEN_INC_FILE"));
}

#[derive(Debug)]
pub enum GetVmmObjFailure {
    NotFound,
    BadSize,
    BadStart,
}

/// A simple wrapper struct to call munmap on `Drop`.
struct Mapped<'a>(&'a [u8]);

impl<'a> Mapped<'_> {
    /// # Safety
    /// - `ptr` must be memory mmap-ed in this process and be readable for `sz` bytes while this struct is live.
    /// - It must be sound to unmap this memory when this struct is dropped.
    unsafe fn new(ptr: *const c_void, sz: usize) -> Self {
        Self(unsafe { core::slice::from_raw_parts(ptr.cast(), sz) })
    }
}

impl Drop for Mapped<'_> {
    fn drop(&mut self) {
        // SAFETY: This struct exists specifically to call munmap on mmap-ed memory. new is marked
        // unsafe to discourage improper construction of this struct and details that ptr must be
        // mapped in this process with the corresponding size.
        let res = unsafe { libc::munmap(self.0.as_ptr() as *mut _, self.0.len()) };
        if res != 0 {
            log::error!("munmap failed!");
        }
    }
}

pub fn get_vmm_obj(name: &CStr) -> Result<Vec<u8>, GetVmmObjFailure> {
    let mut ptr: *const c_void = std::ptr::null();
    let mut sz: usize = 0;
    // SAFETY:
    // - `name.as_ptr()` points to as a valid, readable nul-terminated string as promised by CStr
    // - ptr and sz have been initialized to valid values
    // - references to name, ptr, and sz are not retained.
    let rc = unsafe { crate::sys::vmm_obj_map_ro(name.as_ptr().cast(), &mut ptr, &mut sz) };
    if rc < 0 || ptr.is_null() {
        return Err(GetVmmObjFailure::NotFound);
    }
    // SAFETY: On success, vmm_obj_map_ro sets ptr to point to static immutable memory readable for
    // sz bytes.
    let buffer = unsafe { Mapped::new(ptr, sz) };
    let start = u32::from_ne_bytes(
        buffer
            .0
            .get(..4)
            .ok_or(GetVmmObjFailure::BadStart)?
            .try_into()
            .map_err(|_| GetVmmObjFailure::BadStart)?,
    ) as usize;
    let sz = u32::from_ne_bytes(
        buffer
            .0
            .get(4..8)
            .ok_or(GetVmmObjFailure::BadSize)?
            .try_into()
            .map_err(|_| GetVmmObjFailure::BadSize)?,
    ) as usize;
    Ok(buffer.0.get(start..start + sz).ok_or(GetVmmObjFailure::BadSize)?.to_vec())
}
