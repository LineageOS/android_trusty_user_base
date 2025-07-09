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

package android.trusty.dice.policy.matching;

/**
 * Error codes for AuthMgr policy matching service.
 */
@Backing(type="int")
enum Error {
    /** Success */
    OK = 0,

    /** Given client sequence number not found in the cache */
    CLIENT_SEQ_NUMBER_NOT_FOUND = -1,

    /** Failed to allocate memory */
    MEMORY_ALLOCATION_FAILED = -2,
}