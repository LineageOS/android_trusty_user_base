/*
 * Copyright (C) 2018 The Android Open Source Project
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

#define TLOG_TAG "libtipc"

#include <lib/tipc/tipc.h>

#include <assert.h>
#include <lk/err_ptr.h>
#include <lk/macros.h>
#include <stdio.h>
#include <stdlib.h>
#include <trusty/time.h>
#include <trusty_log.h>
#include <uapi/err.h>

#include "tipc_priv.h"

int tipc_connect(handle_t* handle_p, const char* port) {
    int rc;

    assert(handle_p);

    rc = connect(port, IPC_CONNECT_WAIT_FOR_PORT);
    if (rc < 0)
        return rc;

    *handle_p = (handle_t)rc;
    return 0;
}

/*
 *  Send single buf message
 */
int tipc_send1(handle_t chan, const void* buf, size_t len) {
    struct iovec iov = {
            .iov_base = (void*)buf,
            .iov_len = len,
    };
    ipc_msg_t msg = {
            .iov = &iov,
            .num_iov = 1,
            .handles = NULL,
            .num_handles = 0,
    };
    return send_msg(chan, &msg);
}

/*
 *  Receive single buf message
 */
int tipc_recv1(handle_t chan, size_t min_sz, void* buf, size_t buf_sz) {
    int rc;
    ipc_msg_info_t msg_inf;

    rc = get_msg(chan, &msg_inf);
    if (rc)
        return rc;

    if (msg_inf.len < min_sz || msg_inf.len > buf_sz) {
        /* unexpected msg size: buffer too small or too big */
        rc = ERR_BAD_LEN;
    } else {
        struct iovec iov = {
                .iov_base = buf,
                .iov_len = buf_sz,
        };
        ipc_msg_t msg = {
                .iov = &iov,
                .num_iov = 1,
                .handles = NULL,
                .num_handles = 0,
        };
        rc = read_msg(chan, msg_inf.id, 0, &msg);
    }

    put_msg(chan, msg_inf.id);
    return rc;
}

/*
 * Send message consisting of two segments (header and payload)
 */
int tipc_send2(handle_t chan,
               const void* hdr,
               size_t hdr_len,
               const void* payload,
               size_t payload_len) {
    struct iovec iovs[2] = {
            {
                    .iov_base = (void*)hdr,
                    .iov_len = hdr_len,
            },
            {
                    .iov_base = (void*)payload,
                    .iov_len = payload_len,
            },
    };
    ipc_msg_t msg = {
            .iov = iovs,
            .num_iov = countof(iovs),
            .handles = NULL,
            .num_handles = 0,
    };
    return send_msg(chan, &msg);
}

/*
 * Receive message consisting of two segments.
 */
int tipc_recv2(handle_t chan,
               size_t min_sz,
               void* buf1,
               size_t buf1_sz,
               void* buf2,
               size_t buf2_sz) {
    int rc;
    ipc_msg_info_t msg_inf;

    rc = get_msg(chan, &msg_inf);
    if (rc)
        return rc;

    if (msg_inf.len < min_sz || (msg_inf.len > (buf1_sz + buf2_sz))) {
        /* unexpected msg size: buffer too small or too big */
        rc = ERR_BAD_LEN;
    } else {
        struct iovec iovs[2] = {
                {
                        .iov_base = buf1,
                        .iov_len = buf1_sz,
                },
                {
                        .iov_base = buf2,
                        .iov_len = buf2_sz,
                },
        };
        ipc_msg_t msg = {
                .iov = iovs,
                .num_iov = countof(iovs),
                .handles = NULL,
                .num_handles = 0,
        };
        rc = read_msg(chan, msg_inf.id, 0, &msg);
    }

    put_msg(chan, msg_inf.id);
    return rc;
}

/*
 * Handle common unexpected port events
 */
void tipc_handle_port_errors(const struct uevent* ev) {
    if ((ev->event & IPC_HANDLE_POLL_ERROR) ||
        (ev->event & IPC_HANDLE_POLL_HUP) ||
        (ev->event & IPC_HANDLE_POLL_MSG) ||
        (ev->event & IPC_HANDLE_POLL_SEND_UNBLOCKED)) {
        /* should never happen with port handles */
        TLOGE("error event (0x%x) for port (%d)\n", ev->event, ev->handle);
        abort();
    }
}

/*
 * Handle common unexpected channel events
 */
void tipc_handle_chan_errors(const struct uevent* ev) {
    if ((ev->event & IPC_HANDLE_POLL_ERROR) ||
        (ev->event & IPC_HANDLE_POLL_READY)) {
        /* should never happen for channel handles */
        TLOGE("error event (0x%x) for chan (%d)\n", ev->event, ev->handle);
        abort();
    }
}

/*
 * Initialize an existing tipc_hset
 */
int tipc_hset_init(struct tipc_hset* hset) {
    int rc;

    assert(!IS_ERR(hset) && hset);

    hset->handle = INVALID_IPC_HANDLE;
    hset->work_queue = (struct list_node)LIST_INITIAL_VALUE(hset->work_queue);

    rc = handle_set_create();
    if (rc < 0)
        return rc;

    hset->handle = (handle_t)rc;
    return 0;
}

/*
 * Allocate and initialize new handle set structure
 */
struct tipc_hset* tipc_hset_create(void) {
    struct tipc_hset* hset;

    hset = malloc(sizeof(struct tipc_hset));
    if (!hset)
        return (void*)(uintptr_t)(ERR_NO_MEMORY);

    int rc = tipc_hset_init(hset);
    if (rc < 0) {
        free(hset);
        return (void*)(uintptr_t)(rc);
    }

    return hset;
}

/*
 * Add handle to handle set
 */
int tipc_hset_add_entry(struct tipc_hset* hset,
                        handle_t handle,
                        uint32_t evt_mask,
                        struct tipc_event_handler* evt_handler) {
    struct uevent uevt = {
            .handle = handle,
            .event = evt_mask,
            .cookie = (void*)evt_handler,
    };

    if (IS_ERR(hset) || !hset || !evt_handler)
        return ERR_INVALID_ARGS;

    assert(evt_handler->proc);

    /* attach new entry */
    return handle_set_ctrl(hset->handle, HSET_ADD, &uevt);
}

/*
 * Modify handle set entry
 */
int tipc_hset_mod_entry(struct tipc_hset* hset,
                        handle_t handle,
                        uint32_t evt_mask,
                        struct tipc_event_handler* evt_handler) {
    struct uevent uevt = {
            .handle = handle,
            .event = evt_mask,
            .cookie = (void*)evt_handler,
    };

    if (IS_ERR(hset) || !hset || !evt_handler)
        return ERR_INVALID_ARGS;

    assert(evt_handler->proc);

    /* modify entry */
    return handle_set_ctrl(hset->handle, HSET_MOD, &uevt);
}

/*
 * Remove handle from handle set
 */
int tipc_hset_remove_entry(struct tipc_hset* hset, handle_t h) {
    struct uevent uevt = {
            .handle = h,
            .event = 0,
            .cookie = NULL,
    };

    if (IS_ERR(hset) || !hset)
        return ERR_INVALID_ARGS;

    /* detach entry */
    return handle_set_ctrl(hset->handle, HSET_DEL, &uevt);
}

#define MS_NS_FACTOR 1000000

static uint32_t ms_until_deadline(int64_t deadline_at, int64_t curr_time) {
    if (deadline_at == INT64_MAX) {
        return INFINITE_TIME;
    }
    if (deadline_at <= curr_time) {
        return 0;
    }

    int64_t to_deadline_ns = deadline_at - curr_time;
    int64_t to_deadline_ms = DIV_ROUND_UP(to_deadline_ns, MS_NS_FACTOR);
    if (to_deadline_ms >= (int64_t)INFINITE_TIME) {
        /*
         * The deadline is further away than will fit in a uint32; do a wait
         * that's as long as possible, but not INFINITE_TIME.
         */
        return INFINITE_TIME - 1;
    }

    return to_deadline_ms;
}

/**
 * calc_timeout() - Calculates the timeout to use to wait() for events
 * @timeout_at:   the system time (ns) before tipc_handle_event() should return
 * @next_work_at: the system time (ns) at which the next work queue item should
 *                be handled
 * @curr_time:    the current system time (ns)
 *
 * Return: the timeout to pass to wait()
 */
static uint32_t calc_timeout(int64_t timeout_at,
                             int64_t next_work_at,
                             int64_t curr_time) {
    if (timeout_at < next_work_at) {
        return ms_until_deadline(timeout_at, curr_time);
    }
    return ms_until_deadline(next_work_at, curr_time);
}

static int tipc_handle_work_queue(struct tipc_hset* hset,
                                  int64_t current_time,
                                  int64_t* next_work_time) {
    *next_work_time = INT64_MAX;

    /*
     * The list is in decreasing run_after order, so pop elements from the tail
     * as they're processed. The order is decreasing to simplify the enqueue
     * code; see the explanation in tipc_queue_work_impl below.
     */
    struct tipc_work_todo* todo;
    while ((todo = list_peek_tail_type(&hset->work_queue, struct tipc_work_todo,
                                       node)) != NULL) {
        if (todo->run_after > current_time) {
            /* This todo isn't ready, so neither are any after it. */
            *next_work_time = todo->run_after;
            break;
        }

        list_delete(&todo->node);
        int rc = todo->do_work(todo);
        if (rc != NO_ERROR) {
            TLOGE("failed to handle queued work item\n");
            return rc;
        }
    }

    return NO_ERROR;
}

int tipc_handle_event(struct tipc_hset* hset, uint32_t timeout) {
    int rc;
    struct uevent evt = UEVENT_INITIAL_VALUE(evt);

    if (IS_ERR(hset) || !hset)
        return ERR_INVALID_ARGS;

    int64_t curr_time;
    rc = trusty_gettime(0, &curr_time);
    if (rc != NO_ERROR) {
        TLOGE("failed to gettime\n");
        return rc;
    }
    int64_t timeout_at =
            timeout == INFINITE_TIME
                    ? INT64_MAX
                    : curr_time + MS_NS_FACTOR * ((int64_t)timeout);

    do {
        int64_t next_work_time;
        rc = tipc_handle_work_queue(hset, curr_time, &next_work_time);
        if (rc != NO_ERROR) {
            return rc;
        }

        rc = trusty_gettime(0, &curr_time);
        if (rc != NO_ERROR) {
            TLOGE("failed to gettime\n");
            return rc;
        }

        /* wait for next event up to specified time */
        rc = wait(hset->handle, &evt,
                  calc_timeout(timeout_at, next_work_time, curr_time));
        if (rc != ERR_TIMED_OUT) {
            if (rc == NO_ERROR) {
                /* get handler */
                struct tipc_event_handler* handler = evt.cookie;

                /* invoke it */
                handler->proc(&evt, handler->priv);
            }
            /* got an event (even if it was an error), so return */
            return rc;
        }

        rc = trusty_gettime(0, &curr_time);
        if (rc != NO_ERROR) {
            TLOGE("failed to gettime\n");
            return rc;
        }

    } while (curr_time < timeout_at);

    /* Waited full duration w/o an event; give up. */
    return ERR_TIMED_OUT;
}

int tipc_run_event_loop(struct tipc_hset* hset) {
    int rc;

    do {
        rc = tipc_handle_event(hset, INFINITE_TIME);
    } while (rc == 0);

    assert(rc != ERR_TIMED_OUT);
    return rc;
}

void tipc_queue_work_delayed_abs(struct tipc_hset* hset,
                                 struct tipc_work_todo* todo,
                                 int64_t run_after_ns) {
    todo->run_after = run_after_ns;

    /*
     * Put work_queue in descending order because we expect most of the todos
     * callers add to have a run_after value that's greater than or equal to the
     * median run_after already in the queue, so we want to iterate though the
     * largest values first. There isn't a helper macro that iterates through
     * the list from tail to head, so we have to use descending order and pop
     * elements from the tail as we run them.
     *
     * We want to start iterating with the largest run_after values for
     * performace reasons. Callers adding multiple todos with the same run_after
     * (like if they calculate current_time + delay once and use it for each
     * todo) or increasing run_after values (like if they recalculate
     * current_time + delay for each todo) will be O(1) this way. Decreasing
     * run_after values (which would be O(N) enqueues) seem less likely.
     */
    struct tipc_work_todo* existing;
    list_for_every_entry(&hset->work_queue, existing, struct tipc_work_todo,
                         node) {
        if (todo->run_after >= existing->run_after) {
            list_add_before(&existing->node, &todo->node);
            return;
        }
    }
    list_add_tail(&hset->work_queue, &todo->node);
}

void tipc_queue_work(struct tipc_hset* hset, struct tipc_work_todo* todo) {
    tipc_queue_work_delayed_abs(hset, todo, 0);
}

void tipc_queue_work_delayed_rel(struct tipc_hset* hset,
                                 struct tipc_work_todo* todo,
                                 int64_t ns_from_now) {
    int64_t current_time;
    int rc = trusty_gettime(0, &current_time);
    /* trusty_gettime only fails when passed invalid args */
    assert(rc == NO_ERROR);

    tipc_queue_work_delayed_abs(hset, todo, current_time + ns_from_now);
}

void tipc_cancel_work(struct tipc_work_todo* todo) {
    list_delete(&todo->node);
}
