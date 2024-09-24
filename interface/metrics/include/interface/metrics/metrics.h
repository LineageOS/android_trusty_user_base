/*
 * Copyright 2021, The Android Open Source Project
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#pragma once

#include <stdbool.h>
#include <stdint.h>
#include <trusty/uuid.h>

/**
 * DOC: Metrics
 *
 * Metrics interface provides a way for Android to get Trusty metrics data.
 *
 * Currently, only "push" model is supported. Clients are expected to connect to
 * metrics service, listen for events, e.g. app crash events, and respond to
 * every event with a &struct metrics_req.
 *
 * Communication is driven by metrics service, i.e. requests/responses are all
 * sent from/to metrics service.
 *
 * Note that the type of the event is not known to the client ahead of time.
 *
 * This interface used to be exposed on the METRICS_PORT for Android to listen
 * to the Trusty metrics event.
 * However the METRICS_PORT is deprecated and replaced by ISTATS_SETTER_PORT
 * from `trusty/user/base/interface/stats_setter` allowing asynchronous callback
 * to a Normal-World IStats.aidl service (also see IStatsSetter.aidl under
 * `system/core/trusty/stats/aidl/android/trusty/stats/setter`).
 *
 * This Metrics interface still is used on the METRICS_CONSUMER_PORT, allowing
 * the Trusty kernel errors to be reported to the metrics user-space service,
 * then relayed via the IStats callback.
 */

#define METRICS_PORT "com.android.trusty.metrics"

#define HASH_SIZE_BYTES 64

/**
 * enum metrics_cmd - command identifiers for metrics interface
 * @METRICS_CMD_RESP_BIT:             message is a response
 * @METRICS_CMD_REQ_SHIFT:            number of bits used by @METRICS_CMD_RESP_BIT
 * @METRICS_CMD_REPORT_EVENT_DROP:    report gaps in the event stream
 * @METRICS_CMD_REPORT_CRASH:         report an app crash event
 * @METRICS_CMD_REPORT_EXIT:          report an app exit
 * @METRICS_CMD_REPORT_STORAGE_ERROR: report trusty storage error
 */
enum metrics_cmd {
    METRICS_CMD_RESP_BIT = 1,
    METRICS_CMD_REQ_SHIFT = 1,

    METRICS_CMD_REPORT_EVENT_DROP = (1 << METRICS_CMD_REQ_SHIFT),
    METRICS_CMD_REPORT_CRASH = (2 << METRICS_CMD_REQ_SHIFT),
    METRICS_CMD_REPORT_EXIT = (3 << METRICS_CMD_REQ_SHIFT),
    METRICS_CMD_REPORT_STORAGE_ERROR = (4 << METRICS_CMD_REQ_SHIFT),
};

/**
 * enum metrics_error - metrics error codes
 * @METRICS_NO_ERROR:        no error
 * @METRICS_ERR_UNKNOWN_CMD: unknown or not implemented command
 */
enum metrics_error {
    METRICS_NO_ERROR = 0,
    METRICS_ERR_UNKNOWN_CMD = 1,
};

/**
 * struct metrics_req - common structure for metrics requests
 * @cmd:      command identifier - one of &enum metrics_cmd
 * @reserved: must be 0
 */
struct metrics_req {
    uint32_t cmd;
    uint32_t reserved;
} __attribute__((__packed__));

/**
 * struct metrics_resp - common structure for metrics responses
 * @cmd: command identifier - %METRICS_CMD_RESP_BIT or'ed with a cmd in
 *                            one of &enum metrics_cmd
 * @status: response status, one of &enum metrics_error
 */
struct metrics_resp {
    uint32_t cmd;
    uint32_t status;
} __attribute__((__packed__));

/**
 * struct metrics_report_exit_req - arguments of %METRICS_CMD_REPORT_EXIT
 *                                   requests
 * @app_id: app_id in the form UUID in ascii format
 *          "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
 * @exit_code: architecture-specific exit code
 */
struct metrics_report_exit_req {
    char app_id[UUID_STR_SIZE];
    uint32_t exit_code;
} __attribute__((__packed__));

/**
 * struct metrics_report_crash_req - arguments of %METRICS_CMD_REPORT_CRASH
 *                                   requests
 * @app_id: app_id in the form UUID in ascii format
 *          "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
 * @crash_reason: architecture-specific code representing the reason for the
 *                crash
 * @far: Fault Address Register corresponding to the crash. It is set to 0 and
 *       not always revealed
 * @far_hash: Fault Address Register obfuscated, always revealed
 * @elr: Exception Link Register corresponding to the crash. It is set to 0 and
 *       not always revealed
 * @elr_hash: Exception Link Register obfuscated, always revealed
 * @is_hash: Boolean value indicating whether far and elr have been ob
 */
struct metrics_report_crash_req {
    char app_id[UUID_STR_SIZE];
    uint32_t crash_reason;
    uint64_t far;
    uint8_t far_hash[HASH_SIZE_BYTES];
    uint64_t elr;
    uint8_t elr_hash[HASH_SIZE_BYTES];
    bool is_hash;
} __attribute__((__packed__));

#define METRICS_MAX_APP_ID_LEN 256

#define METRICS_MAX_MSG_SIZE 1024
