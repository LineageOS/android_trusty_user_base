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

#define TLOG_TAG "metrics-consumer"
#include "consumer.h"

#include <interface/metrics/consumer.h>
#include <lib/tipc/tipc.h>
#include <lib/tipc/tipc_srv.h>
#include <metrics_consts.h>
#include <openssl/sha.h>
#include <stddef.h>
#include <string.h>
#include <trusty_log.h>
#include <uapi/err.h>

/*
Current use cases:
1) Metrics daemon
2) Trusty kernel metrics reporter
3) Storage metrics
4) Android metrics test
*/
#define MAX_METRICS_TA_CONNECTIONS 4

static const struct uuid zero_uuid = UUID_INITIAL_VALUE(zero_uuid);

static bool is_zero_uuid(const struct uuid* peer) {
    return equal_uuid(peer, &zero_uuid);
}

static int on_connect(const struct tipc_port* port,
                      handle_t chan,
                      const struct uuid* peer,
                      void** ctx_p) {

    struct srv_state* state = get_srv_state(port);
    if(is_zero_uuid(peer))
    {
        TLOGD("Updating metrics daemon handle :%d\n", chan);
        if(state->ns_handle != INVALID_IPC_HANDLE) {
                close(state->ns_handle);
        }
        state->ns_handle = chan;
    }

    return NO_ERROR;
}

void hash_trusty_metrics(uint64_t metric, uint8_t *output) {
    uint8_t metric_arr[8];

    /* Convert the metric to an array of uint8_t*/
    for (size_t i = 0; i < sizeof(metric_arr); ++i) {
        metric_arr[i] = (metric >> (8 * i)) & 0xFF;
    }

    SHA512(metric_arr, sizeof(metric_arr), output);
}

static int on_message(const struct tipc_port* port, handle_t chan, void* ctx) {
    int rc;
    struct metrics_req req;
    uint8_t msg[METRICS_MAX_MSG_SIZE];

    memset(msg, 0, sizeof(msg));
    int msg_size = tipc_recv1(chan, sizeof(req),  msg, sizeof(msg));
    if (msg_size < 0) {
        TLOGE("failed (%d) to receive metrics event\n", msg_size);
        return msg_size;
    }

    uint32_t cmd;
    cmd = ((struct metrics_req*)msg)->cmd;

    if(cmd == METRICS_CMD_REPORT_CRASH) {
        struct metrics_crash_msg *crash_msg = (struct metrics_crash_msg *)msg;
        if (crash_msg->crash_args.is_hash) {
            hash_trusty_metrics(crash_msg->crash_args.far, crash_msg->crash_args.far_hash);
            hash_trusty_metrics(crash_msg->crash_args.elr, crash_msg->crash_args.elr_hash);
            crash_msg->crash_args.far = 0;
            crash_msg->crash_args.elr = 0;
        }
    }

    // Check if NS metricsd connected, if so forward it there.
    struct srv_state* state = get_srv_state(port);
    if(is_ns_connected(state)) {
        rc = tipc_send1(state->ns_handle, msg, msg_size);
        if (rc < 0) {
            TLOGE("failed (%d) to send metrics event tp NS metricsd\n", rc);
            return rc;
        }
    }
    else {
        TLOGD("NS metrics daemon not connected\n");
    }

    struct metrics_resp resp = {
        .cmd = (cmd | METRICS_CMD_RESP_BIT)
    };

    rc = tipc_send1(chan, &resp, sizeof(resp));
    if (rc < 0) {
        TLOGE("failed (%d) to send metrics event response\n", rc);
        return rc;
    }

    if ((size_t)rc != sizeof(resp)) {
        TLOGE("unexpected number of bytes sent: %d\n", rc);
        return ERR_BAD_LEN;
    }

    return NO_ERROR;
}

int add_metrics_consumer_service(struct tipc_hset* hset, struct srv_state* state) {
    static struct tipc_port_acl port_acl = {
            .flags = IPC_PORT_ALLOW_TA_CONNECT | IPC_PORT_ALLOW_NS_CONNECT,
    };
    static struct tipc_port port = {
            .name = METRICS_CONSUMER_PORT,
            .msg_max_size = METRICS_MAX_MSG_SIZE,
            .msg_queue_len = 1,
            .acl = &port_acl,
    };
    static struct tipc_srv_ops ops = {
            .on_message = on_message,
            .on_connect = on_connect,
    };
    set_srv_state(&port, state);

    return tipc_add_service(hset, &port, 1, MAX_METRICS_TA_CONNECTIONS, &ops);
}