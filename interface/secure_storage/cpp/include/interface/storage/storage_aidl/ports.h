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
#pragma once

// note: this port name shall be constructed as the service name followed by
// the .bnd suffix.
// if length is >= 64bytes, the shortened version shall be used,
// by replacing "android.hardware.security.see" with "ahss".
// here the full name is 64 bytes so we have to use the shortened uri
#define STORAGE_ISECURE_STORAGE_PORT "ahss.storage.ISecureStorage/default.bnd"

// note: this port name shall be constructed as the service name prefixed by
// the "IHandover/" prefix.
// if length is >= 64bytes, the shortened version shall be used,
// by replacing "android.hardware.security.see" with "ahss", and by
// using "hnd" as the suffix.
// here the full name is 64 bytes so we have to use the shortened uri
#define STORAGE_ISECURE_STORAGE_HANDOVER_PORT \
    "ahss.storage.ISecureStorage/default.hnd"
