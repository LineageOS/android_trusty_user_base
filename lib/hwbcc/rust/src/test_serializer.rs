/*
 * Copyright (C) 2025 The Android Open Source Project
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

use tipc::{Handle, Serializer};

#[derive(Debug)]
pub struct TestSerializationError;

/// A simple serializer that copies input data and allocates.
/// Not suitable for production.
#[derive(Default)]
pub struct TestSerializer {
    pub buffers: Vec<u8>,
    pub handles: Vec<Option<Handle>>,
}

impl<'a> Serializer<'a> for TestSerializer {
    type Ok = ();
    type Error = TestSerializationError;

    fn serialize_bytes(&mut self, bytes: &'a [u8]) -> Result<(), TestSerializationError> {
        self.buffers.extend_from_slice(bytes);
        Ok(())
    }

    fn serialize_handle(&mut self, handle: &'a Handle) -> Result<(), TestSerializationError> {
        let h = handle.try_clone().or(Err(TestSerializationError))?;
        self.handles.push(Some(h));
        Ok(())
    }
}
