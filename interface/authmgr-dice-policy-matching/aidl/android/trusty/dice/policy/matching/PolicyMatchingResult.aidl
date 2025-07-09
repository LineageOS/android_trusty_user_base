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
 * An enum representing the result of matching a given DICE policy against the latest pair of
 * DICE chain and policy of a client.
 */
enum PolicyMatchingResult {
    /* The given policy matches the DICE chain and the policy is the latest one */
    SUCCESS,

    /* The given policy matches the DICE chain but it is not the latest policy */
    MATCHED_NOT_CURRENT,

    /* The given policy does not match the DICE chain */
    FAILED,
}