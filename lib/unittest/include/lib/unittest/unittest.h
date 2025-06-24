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

#pragma once

#include <inttypes.h>
#include <lk/compiler.h>
#include <stdbool.h>
#include <trusty_ipc.h>

/*
 * -Wmain warns against using extern "C" for main
 * (it's not supposed to have any linkage),
 * except when -ffreestanding is used.
 * So only declare main with extern "C" when !__STDC_HOSTED__.
 */
#if !__STDC_HOSTED__ && __cplusplus
#define EXTERN_C_MAIN extern "C"
#else
#define EXTERN_C_MAIN
#endif

#define PORT_TEST(suite_name, port_name_string)           \
    __BEGIN_CDECLS                                        \
    static bool run_##suite_name(struct unittest* test) { \
        return RUN_ALL_TESTS();                           \
    }                                                     \
    __END_CDECLS                                          \
                                                          \
    EXTERN_C_MAIN int main(void) {                        \
        static struct unittest test = {                   \
                .port_name = port_name_string,            \
                .run_test = run_##suite_name,             \
        };                                                \
        struct unittest* tests = &test;                   \
        return unittest_main(&tests, 1);                  \
    }                                                     \

__BEGIN_CDECLS

struct unittest {
    const char* port_name;
    bool (*run_test)(struct unittest* test);
    handle_t _port_handle;
};

int unittest_main(struct unittest** tests, size_t test_count);

__END_CDECLS
