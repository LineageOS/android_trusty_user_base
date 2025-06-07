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

package android.trusty.handover;

/**
 * This interface should be implemented by all the trusted services that are available to be
 * accessed by the remote clients.
 * The AuthMgr BE accesses this interface to establish a connection between a remote client and the
 * trusted service, so that the remote client and the trusted service can communicate independently
 * from the AuthMgr, once the AuthMgr authorization protocol is successfully completed.
 */
interface ITrustedServicesHandover {
    /**
     * The AuthMgr BE invokes this method to handover an authorized connection to
     * the trusted service. The AuthMgr authorization protocol defined in `IAuthMgrAuthorization`
     * makes sure that the other end of the connection is handed over to the authorized remote
     * client.
     * In addition to the connection handle, the AuthMgr BE also conveys to the trusted service
     * a unique client identifier.
     *
     * @param connectionHandle - a handle to the new connection that was setup out-of-band between
     *                           the AuthMgr BE and the AuthMgr FE and authorized during phase 2 of
     *                           the AuthMgr authorization protocol
     * @param clientSeqNumber - the unique sequence number assigned to the client
     */
    void handoverConnection(in ParcelFileDescriptor connectionHandle, in long clientSeqNumber);
}
