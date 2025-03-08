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

package trusty.os;

import android.os.ParcelFileDescriptor;

/**
 * Interface for accessing the RPC server of a service.
 * This interface was originally copied from frameworks/native/libs/binder/aidl/android/os/IAccessor.aidl
 * because it serves a similar function, but this interface is for use within trusty only
 * and its use should never assume that the internal binder IAccessor interface is stable or compatible
 * with this one.
 *
 * @hide
 */
interface ITrustyAccessor {
    /**
     * The connection info was not available for this service.
     * This happens when the user-supplied callback fails to produce
     * valid connection info.
     * Depending on the implementation of the callback, it might be helpful
     * to retry.
     */
    const int ERROR_CONNECTION_INFO_NOT_FOUND = 0;
    /**
     * Failed to create the socket. Often happens when the process trying to create
     * the socket lacks the permissions to do so.
     * This may be a temporary issue, so retrying the operation is OK.
     */
    const int ERROR_FAILED_TO_CREATE_SOCKET = 1;
    /**
     * Failed to connect to the socket. This can happen for many reasons, so be sure
     * log the error message and check it.
     * This may be a temporary issue, so retrying the operation is OK.
     */
    const int ERROR_FAILED_TO_CONNECT_TO_SOCKET = 2;
    /**
     * Failed to connect to the socket with EACCES because this process does not
     * have perimssions to connect.
     * There is no need to retry the connection as this access will not be granted
     * upon retry.
     */
    const int ERROR_FAILED_TO_CONNECT_EACCES = 3;
    /**
     * Unsupported socket family type returned.
     * There is no need to retry the connection as this socket family is not
     * supported.
     */
    const int ERROR_UNSUPPORTED_SOCKET_FAMILY = 4;

    /**
     * Adds a connection to the RPC server of the service managed by the ITrustyAccessor.
     *
     * This method can be called multiple times to establish multiple distinct
     * connections to the same RPC server.
     *
     * @throws ServiceSpecificError with message and one of the ITrustyAccessor::ERROR_ values.
     *
     * @return A file descriptor connected to the RPC session of the service managed
     *         by ITrustyAccessor.
     */
    ParcelFileDescriptor addConnection();

    /**
     * Get the instance name for the service this accessor is responsible for.
     *
     * This is used to verify the proxy binder is associated with the expected instance name.
     */
    String getInstanceName();
}
