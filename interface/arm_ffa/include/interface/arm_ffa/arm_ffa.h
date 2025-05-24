/*
 * Copyright (C) 2023 The Android Open Source Project
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

/*
 * Subset of Arm PSA Firmware Framework for Arm v8-A 1.0 EAC
 * (https://developer.arm.com/docs/den0077/a) needed for shared memory.
 */

#include <interface/smc/smc_def.h>
#include <stdint.h>

#ifndef STATIC_ASSERT
#define STATIC_ASSERT(e) _Static_assert(e, #e)
#endif

#define FFA_CURRENT_VERSION_MAJOR (1U)
#define FFA_CURRENT_VERSION_MINOR (2U)

#define FFA_VERSION_TO_MAJOR(version) ((version) >> 16)
#define FFA_VERSION_TO_MINOR(version) ((version) & (0xffff))
#define FFA_VERSION(major, minor) (((major) << 16) | (minor))
#define FFA_CURRENT_VERSION \
    FFA_VERSION(FFA_CURRENT_VERSION_MAJOR, FFA_CURRENT_VERSION_MINOR)

#define SMC_ENTITY_SHARED_MEMORY 4

#define SMC_FASTCALL_NR_SHARED_MEMORY(nr) \
    SMC_FASTCALL_NR(SMC_ENTITY_SHARED_MEMORY, nr)
#define SMC_FASTCALL64_NR_SHARED_MEMORY(nr) \
    SMC_FASTCALL64_NR(SMC_ENTITY_SHARED_MEMORY, nr)

#define FFA_PAGE_SIZE (4096)

/**
 * typedef ffa_endpoint_id16_t - Endpoint ID
 *
 * Current implementation only supports VMIDs. FFA spec also support stream
 * endpoint ids.
 */
typedef uint16_t ffa_endpoint_id16_t;

/**
 * struct ffa_cons_mrd - Constituent memory region descriptor
 * @address:
 *         Start address of contiguous memory region. Must be 4K page aligned.
 * @page_count:
 *         Number of 4K pages in region.
 * @reserved_12_15:
 *         Reserve bytes 12-15 to pad struct size to 16 bytes.
 */
struct ffa_cons_mrd {
    uint64_t address;
    uint32_t page_count;
    uint32_t reserved_12_15;
};
STATIC_ASSERT(sizeof(struct ffa_cons_mrd) == 16);

/**
 * struct ffa_comp_mrd - Composite memory region descriptor
 * @total_page_count:
 *         Number of 4k pages in memory region. Must match sum of
 *         @address_range_array[].page_count.
 * @address_range_count:
 *         Number of entries in @address_range_array.
 * @reserved_8_15:
 *         Reserve bytes 8-15 to pad struct size to 16 byte alignment and
 *         make @address_range_array 16 byte aligned.
 * @address_range_array:
 *         Array of &struct ffa_cons_mrd entries.
 */
struct ffa_comp_mrd {
    uint32_t total_page_count;
    uint32_t address_range_count;
    uint64_t reserved_8_15;
    struct ffa_cons_mrd address_range_array[];
};
STATIC_ASSERT(sizeof(struct ffa_comp_mrd) == 16);

/**
 * typedef ffa_mem_attr8_t - Memory region attributes
 *
 * * @FFA_MEM_ATTR_DEVICE_NGNRNE:
 *     Device-nGnRnE.
 * * @FFA_MEM_ATTR_DEVICE_NGNRE:
 *     Device-nGnRE.
 * * @FFA_MEM_ATTR_DEVICE_NGRE:
 *     Device-nGRE.
 * * @FFA_MEM_ATTR_DEVICE_GRE:
 *     Device-GRE.
 * * @FFA_MEM_ATTR_NORMAL_MEMORY_UNCACHED
 *     Normal memory. Non-cacheable.
 * * @FFA_MEM_ATTR_NORMAL_MEMORY_CACHED_WB
 *     Normal memory. Write-back cached.
 * * @FFA_MEM_ATTR_NON_SHAREABLE
 *     Non-shareable. Combine with FFA_MEM_ATTR_NORMAL_MEMORY_*.
 * * @FFA_MEM_ATTR_OUTER_SHAREABLE
 *     Outer Shareable. Combine with FFA_MEM_ATTR_NORMAL_MEMORY_*.
 * * @FFA_MEM_ATTR_INNER_SHAREABLE
 *     Inner Shareable. Combine with FFA_MEM_ATTR_NORMAL_MEMORY_*.
 * * @FFA_MEM_ATTR_NONSECURE
 *     Nonsecure.
 *     Uses reserved bit 6, the same bit which a NONSECURE flag is in a
 *     spec draft.
 */
typedef uint8_t ffa_mem_attr8_t;
#define FFA_MEM_ATTR_DEVICE_NGNRNE ((1U << 4) | (0x0U << 2))
#define FFA_MEM_ATTR_DEVICE_NGNRE ((1U << 4) | (0x1U << 2))
#define FFA_MEM_ATTR_DEVICE_NGRE ((1U << 4) | (0x2U << 2))
#define FFA_MEM_ATTR_DEVICE_GRE ((1U << 4) | (0x3U << 2))
#define FFA_MEM_ATTR_NORMAL_MEMORY_UNCACHED ((2U << 4) | (0x1U << 2))
#define FFA_MEM_ATTR_NORMAL_MEMORY_CACHED_WB ((2U << 4) | (0x3U << 2))
#define FFA_MEM_ATTR_NON_SHAREABLE (0x0U << 0)
#define FFA_MEM_ATTR_OUTER_SHAREABLE (0x2U << 0)
#define FFA_MEM_ATTR_INNER_SHAREABLE (0x3U << 0)
#define FFA_MEM_ATTR_NONSECURE (1U << 6)

/**
 * typedef ffa_mem_perm8_t - Memory access permissions
 *
 * * @FFA_MEM_ATTR_RO
 *     Request or specify read-only mapping.
 * * @FFA_MEM_ATTR_RW
 *     Request or allow read-write mapping.
 * * @FFA_MEM_PERM_NX
 *     Deny executable mapping.
 * * @FFA_MEM_PERM_X
 *     Request executable mapping.
 */
typedef uint8_t ffa_mem_perm8_t;
#define FFA_MEM_PERM_RO (1U << 0)
#define FFA_MEM_PERM_RW (1U << 1)
#define FFA_MEM_PERM_NX (1U << 2)
#define FFA_MEM_PERM_X (1U << 3)

/**
 * typedef ffa_mem_flag8_t - Endpoint memory flags
 *
 * * @FFA_MEM_FLAG_NON_RETRIEVAL_BORROWER
 *     Non-retrieval Borrower. Memory region must not be or was not retrieved on
 *     behalf of this endpoint.
 */
typedef uint8_t ffa_mem_flag8_t;
#define FFA_MEM_FLAG_NON_RETRIEVAL_BORROWER (1U << 0)

/**
 * typedef ffa_mtd_flag32_t - Memory transaction descriptor flags
 *
 * * @FFA_MTD_FLAG_ZERO_MEMORY
 *     Zero memory after unmapping from sender (must be 0 for share).
 * * @FFA_MTD_FLAG_TIME_SLICING
 *     Not supported by this implementation.
 * * @FFA_MTD_FLAG_ZERO_MEMORY_AFTER_RELINQUISH
 *     Zero memory after unmapping from borrowers (must be 0 for share).
 * * @FFA_MTD_FLAG_TYPE_MASK
 *     Bit-mask to extract memory management transaction type from flags.
 * * @FFA_MTD_FLAG_TYPE_SHARE_MEMORY
 *     Share memory transaction flag.
 *     Used by @SMC_FC_FFA_MEM_RETRIEVE_RESP to indicate that memory came from
 *     @SMC_FC_FFA_MEM_SHARE and by @SMC_FC_FFA_MEM_RETRIEVE_REQ to specify that
 *     it must have.
 * * @FFA_MTD_FLAG_TYPE_LEND_MEMORY
 *     Lend memory transaction flag.
 *     Used by @SMC_FC_FFA_MEM_RETRIEVE_RESP to indicate that memory came from
 *     @SMC_FC_FFA_MEM_LEND and by @SMC_FC_FFA_MEM_RETRIEVE_REQ to specify that
 *     it must have.
 * * @FFA_MTD_FLAG_TYPE_DONATE_MEMORY
 *     Donate memory transaction flag.
 *     Used by @SMC_FC_FFA_MEM_RETRIEVE_RESP to indicate that memory came from
 *     @SMC_FC_FFA_MEM_DONATE and by @SMC_FC_FFA_MEM_RETRIEVE_REQ to specify
 *     that it must have.
 * * @FFA_MTD_FLAG_ADDRESS_RANGE_ALIGNMENT_HINT_MASK
 *     Not supported by this implementation.
 */
typedef uint32_t ffa_mtd_flag32_t;
#define FFA_MTD_FLAG_ZERO_MEMORY (1U << 0)
#define FFA_MTD_FLAG_TIME_SLICING (1U << 1)
#define FFA_MTD_FLAG_ZERO_MEMORY_AFTER_RELINQUISH (1U << 2)
#define FFA_MTD_FLAG_TYPE_MASK (3U << 3)
#define FFA_MTD_FLAG_TYPE_SHARE_MEMORY (1U << 3)
#define FFA_MTD_FLAG_TYPE_LEND_MEMORY (2U << 3)
#define FFA_MTD_FLAG_TYPE_DONATE_MEMORY (3U << 3)
#define FFA_MTD_FLAG_ADDRESS_RANGE_ALIGNMENT_HINT_MASK (0x1FU << 5)

/**
 * typedef ffa_mem_relinquish_flag32_t - Memory relinquish descriptor flags
 *
 * * @FFA_MEM_RELINQUISH_FLAG_ZERO_MEMORY
 *     Zero memory after unmapping from sender (must be 0 for share).
 * * @FFA_MEM_RELINQUISH_FLAG_TIME_SLICING
 *     Not supported by this implementation.
 */
typedef uint32_t ffa_mem_relinquish_flag32_t;
#define FFA_MEM_RELINQUISH_FLAG_ZERO_MEMORY (1U << 0)
#define FFA_MEM_RELINQUISH_FLAG_TIME_SLICING (1U << 1)

/**
 * struct ffa_mapd - Memory access permissions descriptor
 * @endpoint_id:
 *         Endpoint id that @memory_access_permissions and @flags apply to.
 *         (&typedef ffa_endpoint_id16_t).
 * @memory_access_permissions:
 *         FFA_MEM_PERM_* values or'ed together (&typedef ffa_mem_perm8_t).
 * @flags:
 *         FFA_MEM_FLAG_* values or'ed together (&typedef ffa_mem_flag8_t).
 */
struct ffa_mapd {
    ffa_endpoint_id16_t endpoint_id;
    ffa_mem_perm8_t memory_access_permissions;
    ffa_mem_flag8_t flags;
};
STATIC_ASSERT(sizeof(struct ffa_mapd) == 4);

/**
 * struct ffa_emad - Endpoint memory access descriptor.
 * @mapd:  &struct ffa_mapd.
 * @comp_mrd_offset:
 *         Offset of &struct ffa_comp_mrd form start of &struct ffa_mtd_common.
 * @reserved_8_15:
 *         Reserved bytes 8-15. Must be 0.
 */
struct ffa_emad {
    struct ffa_mapd mapd;
    uint32_t comp_mrd_offset;
    uint64_t reserved_8_15;
};
STATIC_ASSERT(sizeof(struct ffa_emad) == 16);

/**
 * struct ffa_mtd_common - Memory transaction descriptor.
 * @sender_id:
 *         Sender endpoint id.
 * @memory_region_attributes:
 *         FFA_MEM_ATTR_* values or'ed together (&typedef ffa_mem_attr8_t).
 * @reserved_3:
 *         Reserved bytes 3. Must be 0.
 * @flags:
 *         FFA_MTD_FLAG_* values or'ed together (&typedef ffa_mtd_flag32_t).
 * @handle:
 *         Id of shared memory object. Most be 0 for MEM_SHARE.
 * @tag:   Client allocated tag. Must match original value.
 */
struct ffa_mtd_common {
    ffa_endpoint_id16_t sender_id;
    ffa_mem_attr8_t memory_region_attributes;
    uint8_t reserved_3;
    ffa_mtd_flag32_t flags;
    uint64_t handle;
    uint64_t tag;
};
STATIC_ASSERT(sizeof(struct ffa_mtd_common) == 24);

/**
 * struct ffa_mtd_v1_0 - Memory transaction descriptor for v1.0.
 * @common:
 *         The common part of the descriptor (shared with v1.1).
 * @reserved_24_27:
 *         Reserved bytes 24-27. Must be 0.
 * @emad_count:
 *         Number of entries in @emad. Must be 1 in current implementation.
 *         FFA spec allows more entries.
 * @emad:
 *         Endpoint memory access descriptor array (see @struct ffa_emad).
 */
struct ffa_mtd_v1_0 {
    struct ffa_mtd_common common;
    uint32_t reserved_24_27;
    uint32_t emad_count;
    struct ffa_emad emad[];
};
STATIC_ASSERT(sizeof(struct ffa_mtd_v1_0) == 32);

/**
 * struct ffa_mtd_v1_1 - Memory transaction descriptor for v1.1 and higher.
 * @common:
 *         The common part of the descriptor (shared with v1.0).
 * @emad_size:
 *         Size of each endpoint memory access descriptor.
 * @emad_count:
 *         Number of entries at @emad_offset.
 *         Must be 1 in current implementation.
 *         FFA spec allows more entries.
 * @emad_offset:
 *         Offset from the base address of this descriptor to the
 *         endpoint memory access descriptor array (see @struct ffa_emad).
 * @reserved_36: Reserved bytes at offset 36. Must be 0.
 * @reserved_40: Reserved bytes at offset 40. Must be 0.
 */
struct ffa_mtd_v1_1 {
    struct ffa_mtd_common common;
    uint32_t emad_size;
    uint32_t emad_count;
    uint32_t emad_offset;
    uint32_t reserved_36;
    uint64_t reserved_40;
};
STATIC_ASSERT(sizeof(struct ffa_mtd_v1_1) == 48);

/**
 * struct ffa_mem_relinquish_descriptor - Relinquish request descriptor.
 * @handle:
 *         Id of shared memory object to relinquish.
 * @flags:
 *         FFA_MEM_RELINQUISH_FLAG_* values or'ed together
 *         (&typedef ffa_mem_relinquish_flag32_t).
 * @endpoint_count:
 *         Number of entries in @endpoint_array.
 * @endpoint_array:
 *         Array of endpoint ids.
 */
struct ffa_mem_relinquish_descriptor {
    uint64_t handle;
    uint32_t flags;
    uint32_t endpoint_count;
    ffa_endpoint_id16_t endpoint_array[];
};
STATIC_ASSERT(sizeof(struct ffa_mem_relinquish_descriptor) == 16);

/**
 * typedef ffa_features2_t - FFA_FEATURES values returned in w2
 *
 * * @FFA_FEATURES2_RXTX_MAP_BUF_SIZE_MASK
 *     For RXTX_MAP: min buffer size and alignment boundary mask.
 * * @FFA_FEATURES2_RXTX_MAP_BUF_SIZE_4K
 *     For RXTX_MAP: min buffer size and alignment boundary is 4K.
 * * @FFA_FEATURES2_RXTX_MAP_BUF_SIZE_64K
 *     For RXTX_MAP: min buffer size and alignment boundary is 64K.
 * * @FFA_FEATURES2_RXTX_MAP_BUF_SIZE_16K
 *     For RXTX_MAP: min buffer size and alignment boundary is 16K.
 * * @FFA_FEATURES2_MEM_DYNAMIC_BUFFER
 *     Supports custom buffers for memory transactions.
 * * @FFA_FEATURES2_MEM_RETRIEVE_REQ_NS_BIT
 *     Supports setting the NS bit on retrieved descriptors.
 *
 * For all other bits and commands: must be 0.
 */
typedef uint32_t ffa_features2_t;
#define FFA_FEATURES2_RXTX_MAP_BUF_SIZE_MASK 0x3U
#define FFA_FEATURES2_RXTX_MAP_BUF_SIZE_4K 0x0U
#define FFA_FEATURES2_RXTX_MAP_BUF_SIZE_64K 0x1U
#define FFA_FEATURES2_RXTX_MAP_BUF_SIZE_16K 0x2U
#define FFA_FEATURES2_MEM_DYNAMIC_BUFFER 0x1U
#define FFA_FEATURES2_MEM_RETRIEVE_REQ_NS_BIT 0x2U

/**
 * typedef ffa_features3_t - FFA_FEATURES values returned in w3
 *
 * * @FFA_FEATURES3_MEM_RETRIEVE_REQ_REFCOUNT_MASK
 *     For FFA_MEM_RETRIEVE_REQ, bit[7-0]: Number of times receiver can
 *     retrieve each memory region before relinquishing it specified as
 *     ((1U << (value + 1)) - 1 (or value = bits in reference count - 1).
 *
 * For all other bits and commands: must be 0.
 */
typedef uint32_t ffa_features3_t;
#define FFA_FEATURES3_MEM_RETRIEVE_REQ_REFCOUNT_MASK 0xffU

/**
 * Flags passed to FFA_PARTITION_INFO_GET
 *
 * * @FFA_PARTITION_INFO_GET_FLAG_RETURN_COUNT_ONLY
 *     Return only the count of partitions corresponding to the given UUID.
 */
typedef uint32_t ffa_partition_info_get_flag32_t;
#define FFA_PARTITION_INFO_GET_FLAG_RETURN_COUNT_ONLY (1U << 0)

struct ffa_part_info_desc {
    uint16_t partition_id;
    uint16_t exec_ctx_or_proxy_id;
    uint32_t properties;
    uint64_t uuid_lo;
    uint64_t uuid_hi;
};
STATIC_ASSERT(sizeof(struct ffa_part_info_desc) == 24);

/**
 * typedef ffa_msg_wait_flag32_t - FFA_MSG_WAIT flags
 *
 * * @FFA_MSG_WAIT_FLAG_RETAIN_RX
 *     Retain RX Buffer Ownership flag.
 */
typedef uint32_t ffa_msg_wait_flag32_t;
#define FFA_MSG_WAIT_FLAG_RETAIN_RX (1U << 0)

/**
 * enum ffa_error - FF-A error code
 * @FFA_ERROR_NOT_SUPPORTED:
 *         Operation is not supported by the current implementation.
 * @FFA_ERROR_INVALID_PARAMETERS:
 *         Invalid parameters. Conditions function specific.
 * @FFA_ERROR_NO_MEMORY:
 *         Not enough memory.
 * @FFA_ERROR_BUSY:
 *         Operation temporarily not possible. Conditions function specific.
 * @FFA_ERROR_INTERRUPTED:
 *         This error code is not specified in the FF-A specification.
 * @FFA_ERROR_DENIED:
 *         Operation not allowed. Conditions function specific.
 * @FFA_ERROR_RETRY:
 *         Operation temporarily not possible. Conditions function specific.
 * @FFA_ERROR_ABORTED:
 *         Operation aborted. Reason for abort is implementation specific.
 * @FFA_ERROR_NO_DATA:
 *         Requested information not available.
 * @FFA_ERROR_NOT_READY:
 *         Callee is not ready to handle the request.
 *
 */
enum ffa_error {
    FFA_ERROR_NOT_SUPPORTED = -1,
    FFA_ERROR_INVALID_PARAMETERS = -2,
    FFA_ERROR_NO_MEMORY = -3,
    FFA_ERROR_BUSY = -4,
    FFA_ERROR_INTERRUPTED = -5,
    FFA_ERROR_DENIED = -6,
    FFA_ERROR_RETRY = -7,
    FFA_ERROR_ABORTED = -8,
    FFA_ERROR_NO_DATA = -9,
    FFA_ERROR_NOT_READY = -10,
};

/**
 * SMC_FC32_FFA_MIN - First 32 bit SMC opcode reserved for FFA
 */
#define SMC_FC32_FFA_MIN SMC_FASTCALL_NR_SHARED_MEMORY(0x60)

/**
 * SMC_FC32_FFA_MAX - Last 32 bit SMC opcode reserved for FFA
 */
#define SMC_FC32_FFA_MAX SMC_FASTCALL_NR_SHARED_MEMORY(0x7F)

/**
 * SMC_FC64_FFA_MIN - First 64 bit SMC opcode reserved for FFA
 */
#define SMC_FC64_FFA_MIN SMC_FASTCALL64_NR_SHARED_MEMORY(0x60)

/**
 * SMC_FC64_FFA_MAX - Last 64 bit SMC opcode reserved for FFA
 */
#define SMC_FC64_FFA_MAX SMC_FASTCALL64_NR_SHARED_MEMORY(0x7F)

/**
 * SMC_FC_FFA_ERROR - SMC error return opcode
 *
 * Register arguments:
 *
 * * w1:     VMID in [31:16], vCPU in [15:0]
 * * w2:     Error code (&enum ffa_error)
 */
#define SMC_FC_FFA_ERROR SMC_FASTCALL_NR_SHARED_MEMORY(0x60)

/**
 * SMC_FC_FFA_SUCCESS - 32 bit SMC success return opcode
 *
 * Register arguments:
 *
 * * w1:     VMID in [31:16], vCPU in [15:0]
 * * w2-w7:  Function specific
 */
#define SMC_FC_FFA_SUCCESS SMC_FASTCALL_NR_SHARED_MEMORY(0x61)

/**
 * SMC_FC64_FFA_SUCCESS - 64 bit SMC success return opcode
 *
 * Register arguments:
 *
 * * w1:             VMID in [31:16], vCPU in [15:0]
 * * w2/x2-w7/x7:    Function specific
 */
#define SMC_FC64_FFA_SUCCESS SMC_FASTCALL64_NR_SHARED_MEMORY(0x61)

/**
 * SMC_FC_FFA_INTERRUPT - SMC interrupt return opcode
 *
 * Register arguments:
 *
 * * w1:     VMID in [31:16], vCPU in [15:0]
 * * w2:     Interrupt ID
 */
#define SMC_FC_FFA_INTERRUPT SMC_FASTCALL_NR_SHARED_MEMORY(0x62)

/**
 * SMC_FC_FFA_VERSION - SMC opcode to return supported FF-A version
 *
 * Register arguments:
 *
 * * w1:     Major version bit[30:16] and minor version in bit[15:0] supported
 *           by caller. Bit[31] must be 0.
 *
 * Return:
 * * w0:     &SMC_FC_FFA_SUCCESS
 * * w2:     Major version bit[30:16], minor version in bit[15:0], bit[31] must
 *           be 0.
 *
 * or
 *
 * * w0:     &SMC_FC_FFA_ERROR
 * * w2:     %FFA_ERROR_NOT_SUPPORTED if major version passed in is less than
 *           the minimum major version supported.
 */
#define SMC_FC_FFA_VERSION SMC_FASTCALL_NR_SHARED_MEMORY(0x63)

/**
 * SMC_FC_FFA_FEATURES - SMC opcode to check optional feature support
 *
 * Register arguments:
 *
 * * w1:     FF-A function ID
 *
 * Return:
 * * w0:     &SMC_FC_FFA_SUCCESS
 * * w2:     &typedef ffa_features2_t
 * * w3:     &typedef ffa_features3_t
 *
 * or
 *
 * * w0:     &SMC_FC_FFA_ERROR
 * * w2:     %FFA_ERROR_NOT_SUPPORTED if function is not implemented, or
 *           %FFA_ERROR_INVALID_PARAMETERS if function id is not valid.
 */
#define SMC_FC_FFA_FEATURES SMC_FASTCALL_NR_SHARED_MEMORY(0x64)

/**
 * SMC_FC_FFA_RX_RELEASE - SMC opcode to Relinquish ownership of a RX buffer
 *
 * Return:
 * * w0:     &SMC_FC_FFA_SUCCESS
 *
 * or
 *
 * * w0:     &SMC_FC_FFA_ERROR
 * * w2:     %FFA_ERROR_DENIED Caller did not have ownership of the RX buffer.
 *           %FFA_ERROR_NOT_SUPPORTED if operation not supported
 */
#define SMC_FC_FFA_RX_RELEASE SMC_FASTCALL_NR_SHARED_MEMORY(0x65)

/**
 * SMC_FC_FFA_RXTX_MAP - 32 bit SMC opcode to map message buffers
 *
 * Register arguments:
 *
 * * w1:     TX address
 * * w2:     RX address
 * * w3:     RX/TX page count in bit[5:0]
 *
 * Return:
 * * w0:     &SMC_FC_FFA_SUCCESS
 */
#define SMC_FC_FFA_RXTX_MAP SMC_FASTCALL_NR_SHARED_MEMORY(0x66)

/**
 * SMC_FC64_FFA_RXTX_MAP - 64 bit SMC opcode to map message buffers
 *
 * Register arguments:
 *
 * * x1:     TX address
 * * x2:     RX address
 * * x3:     RX/TX page count in bit[5:0]
 *
 * Return:
 * * w0:     &SMC_FC_FFA_SUCCESS
 */
#define SMC_FC64_FFA_RXTX_MAP SMC_FASTCALL64_NR_SHARED_MEMORY(0x66)

/**
 * SMC_FC_FFA_RXTX_UNMAP - SMC opcode to unmap message buffers
 *
 * Register arguments:
 *
 * * w1:     ID in [31:16]
 *
 * Return:
 * * w0:     &SMC_FC_FFA_SUCCESS
 */
#define SMC_FC_FFA_RXTX_UNMAP SMC_FASTCALL_NR_SHARED_MEMORY(0x67)

/**
 * SMC_FC_FFA_PARTITION_INFO_GET - SMC opcode to get info about FF-A components
 *
 * Register arguments:
 *
 * w1:      Bytes[0..3] of UUID in little-endian.
 * w2:      Bytes[4..7] of UUID in little-endian.
 * w3:      Bytes[8..11] of UUID in little-endian.
 * w4:      Bytes[12..15] of UUID in little-endian.
 * w5:      bit[0]    : Information type flag. 1 for just the partition count
 *                      with the UUID and 0 for the partition info descriptors.
 *          bit[31:1] : SBZ
 * w6-w7:   Reserved (SBZ).
 *
 */
#define SMC_FC_FFA_PARTITION_INFO_GET SMC_FASTCALL_NR_SHARED_MEMORY(0x68)

/**
 * SMC_FC_FFA_ID_GET - SMC opcode to get endpoint id of caller
 *
 * Return:
 * * w0:     &SMC_FC_FFA_SUCCESS
 * * w2:     ID in bit[15:0], bit[31:16] must be 0.
 */
#define SMC_FC_FFA_ID_GET SMC_FASTCALL_NR_SHARED_MEMORY(0x69)

/**
 * SMC_FC_FFA_MSG_WAIT - SMC opcode to transition from running to waiting state
 */
#define SMC_FC_FFA_MSG_WAIT SMC_FASTCALL_NR_SHARED_MEMORY(0x6B)

/**
 * SMC_FC_FFA_YIELD - SMC opcode to yield execution back to the component that
 *                    scheduled it.
 *
 * Register arguments:
 *
 * * w1:     Endpoint ID in [31:16], vCPU in [15:0].
 * * w2:     Lower 32-bits of timeout interval in nanoseconds after which vCPU
 *           in w1 must run.
 * * w3:     Upper 32-bits of timeout interval in nanoseconds after which vCPU
 *           in w1 must run.
 */
#define SMC_FC_FFA_YIELD SMC_FASTCALL_NR_SHARED_MEMORY(0x6C)

/**
 * SMC_FC_FFA_MSG_RUN - SMC opcode to allocate cycles to an endpoint
 *
 * Register arguments:
 *
 * * w1:     SP/VM ID in [31:16], vCPU in [15:0]
 */
#define SMC_FC_FFA_RUN SMC_FASTCALL_NR_SHARED_MEMORY(0x6D)

/**
 * SMC_FC_FFA_MSG_SEND_DIRECT_REQ - 32 bit SMC opcode to send direct message as
 *                                  a request
 *
 * Register arguments:
 *
 * * w1:     Sender ID in bit[31:16], receiver ID in [15:0]
 * * w2:     Message Flags.
 *           bit[31]   : Message type. 0 for partition message and 1 for
 *                       framework message.
 *           bit[30:8] : Reserved. Must be 0.
 *           bit[7:0]  : Framework message type. Must be 0 if partition message.
 * * w3-w7:  Implementation defined.
 */
#define SMC_FC_FFA_MSG_SEND_DIRECT_REQ SMC_FASTCALL_NR_SHARED_MEMORY(0x6F)

/**
 * SMC_FC64_FFA_MSG_SEND_DIRECT_REQ - 64 bit SMC opcode to send direct message
 *                                    as a request
 *
 * Register arguments:
 *
 * * w1:     Sender ID in bit[31:16], receiver ID in [15:0]
 * * w2:     Message Flags.
 *           bit[31]   : Message type. 0 for partition message and 1 for
 *                       framework message.
 *           bit[30:8] : Reserved. Must be 0.
 *           bit[7:0]  : Framework message type. Must be 0 if partition message.
 * * x3-x7:  Implementation defined.
 */
#define SMC_FC64_FFA_MSG_SEND_DIRECT_REQ SMC_FASTCALL64_NR_SHARED_MEMORY(0x6F)

/**
 * SMC_FC_FFA_MSG_SEND_DIRECT_RESP - 32 bit SMC opcode to send direct message as
 *                                   a response
 *
 * Register arguments:
 *
 * * w1:     Sender ID in bit[31:16], receiver ID in [15:0]
 * * w2:     Message Flags.
 *           bit[31]   : Message type. 0 for partition message and 1 for
 *                       framework message.
 *           bit[30:8] : Reserved. Must be 0.
 *           bit[7:0]  : Framework message type. Must be 0 if partition message.
 * * w3-w7:  Implementation defined.
 */
#define SMC_FC_FFA_MSG_SEND_DIRECT_RESP SMC_FASTCALL_NR_SHARED_MEMORY(0x70)

/**
 * SMC_FC64_FFA_MSG_SEND_DIRECT_RESP - 64 bit SMC opcode to send direct message
 *                                     as a response
 *
 * Register arguments:
 *
 * * w1:     Sender ID in bit[31:16], receiver ID in [15:0]
 * * w2:     Message Flags.
 *           bit[31]   : Message type. 0 for partition message and 1 for
 *                       framework message.
 *           bit[30:8] : Reserved. Must be 0.
 *           bit[7:0]  : Framework message type. Must be 0 if partition message.
 * * x3-x7:  Implementation defined.
 */
#define SMC_FC64_FFA_MSG_SEND_DIRECT_RESP SMC_FASTCALL64_NR_SHARED_MEMORY(0x70)

/**
 * SMC_FC64_FFA_MSG_SEND_DIRECT_REQ2 - 64 bit SMC opcode to send direct message
 *                                     as a request
 *
 * Register arguments:
 *
 * * w1:     Sender ID in bit[31:16], receiver ID in [15:0]
 * * x2:     Bytes[0..7] of UUID.
 * * x3:     Bytes[8..15] of UUID.
 * * x4-x17:  Implementation defined.
 */
#define SMC_FC64_FFA_MSG_SEND_DIRECT_REQ2 SMC_FASTCALL64_NR_SHARED_MEMORY(0x8D)

/**
 * SMC_FC64_FFA_MSG_SEND_DIRECT_RESP2 - 64 bit SMC opcode to send direct message
 *                                     as a response
 *
 * Register arguments:
 *
 * * w1:     Sender ID in bit[31:16], receiver ID in [15:0]
 * * w2:     Should be zero.
 * * w3:     Should be zero.
 * * x4-x17:  Implementation defined.
 */
#define SMC_FC64_FFA_MSG_SEND_DIRECT_RESP2 SMC_FASTCALL64_NR_SHARED_MEMORY(0x8E)

/**
 * SMC_FC_FFA_MEM_DONATE - 32 bit SMC opcode to donate memory
 *
 * Not supported.
 */
#define SMC_FC_FFA_MEM_DONATE SMC_FASTCALL_NR_SHARED_MEMORY(0x71)

/**
 * SMC_FC_FFA_MEM_LEND - 32 bit SMC opcode to lend memory
 *
 * Not currently supported.
 */
#define SMC_FC_FFA_MEM_LEND SMC_FASTCALL_NR_SHARED_MEMORY(0x72)

/**
 * SMC_FC_FFA_MEM_SHARE - 32 bit SMC opcode to share memory
 *
 * Register arguments:
 *
 * * w1:     Total length
 * * w2:     Fragment length
 * * w3:     Address
 * * w4:     Page count
 *
 * Return:
 * * w0:     &SMC_FC_FFA_SUCCESS
 * * w2/w3:  Handle
 *
 * or
 *
 * * w0:     &SMC_FC_FFA_MEM_FRAG_RX
 * * w1-:    See &SMC_FC_FFA_MEM_FRAG_RX
 *
 * or
 *
 * * w0:     &SMC_FC_FFA_ERROR
 * * w2:     Error code (&enum ffa_error)
 */
#define SMC_FC_FFA_MEM_SHARE SMC_FASTCALL_NR_SHARED_MEMORY(0x73)

/**
 * SMC_FC64_FFA_MEM_SHARE - 64 bit SMC opcode to share memory
 *
 * Register arguments:
 *
 * * w1:     Total length
 * * w2:     Fragment length
 * * x3:     Address
 * * w4:     Page count
 *
 * Return:
 * * w0:     &SMC_FC_FFA_SUCCESS
 * * w2/w3:  Handle
 *
 * or
 *
 * * w0:     &SMC_FC_FFA_MEM_FRAG_RX
 * * w1-:    See &SMC_FC_FFA_MEM_FRAG_RX
 *
 * or
 *
 * * w0:     &SMC_FC_FFA_ERROR
 * * w2:     Error code (&enum ffa_error)
 */
#define SMC_FC64_FFA_MEM_SHARE SMC_FASTCALL64_NR_SHARED_MEMORY(0x73)

/**
 * SMC_FC_FFA_MEM_RETRIEVE_REQ - 32 bit SMC opcode to retrieve shared memory
 *
 * Register arguments:
 *
 * * w1:     Total length
 * * w2:     Fragment length
 * * w3:     Address
 * * w4:     Page count
 *
 * Return:
 * * w0:             &SMC_FC_FFA_MEM_RETRIEVE_RESP
 * * w1/x1-w5/x5:    See &SMC_FC_FFA_MEM_RETRIEVE_RESP
 */
#define SMC_FC_FFA_MEM_RETRIEVE_REQ SMC_FASTCALL_NR_SHARED_MEMORY(0x74)

/**
 * SMC_FC64_FFA_MEM_RETRIEVE_REQ - 64 bit SMC opcode to retrieve shared memory
 *
 * Register arguments:
 *
 * * w1:     Total length
 * * w2:     Fragment length
 * * x3:     Address
 * * w4:     Page count
 *
 * Return:
 * * w0:             &SMC_FC_FFA_MEM_RETRIEVE_RESP
 * * w1/x1-w5/x5:    See &SMC_FC_FFA_MEM_RETRIEVE_RESP
 */
#define SMC_FC64_FFA_MEM_RETRIEVE_REQ SMC_FASTCALL64_NR_SHARED_MEMORY(0x74)

/**
 * SMC_FC_FFA_MEM_RETRIEVE_RESP - Retrieve 32 bit SMC return opcode
 *
 * Register arguments:
 *
 * * w1:     Total length
 * * w2:     Fragment length
 */
#define SMC_FC_FFA_MEM_RETRIEVE_RESP SMC_FASTCALL_NR_SHARED_MEMORY(0x75)

/**
 * SMC_FC_FFA_MEM_RELINQUISH - SMC opcode to relinquish shared memory
 *
 * Input in &struct ffa_mem_relinquish_descriptor format in message buffer.
 *
 * Return:
 * * w0:     &SMC_FC_FFA_SUCCESS
 */
#define SMC_FC_FFA_MEM_RELINQUISH SMC_FASTCALL_NR_SHARED_MEMORY(0x76)

/**
 * SMC_FC_FFA_MEM_RECLAIM - SMC opcode to reclaim shared memory
 *
 * Register arguments:
 *
 * * w1/w2:  Handle
 * * w3:     Flags
 *
 * Return:
 * * w0:     &SMC_FC_FFA_SUCCESS
 */
#define SMC_FC_FFA_MEM_RECLAIM SMC_FASTCALL_NR_SHARED_MEMORY(0x77)

/**
 * SMC_FC_FFA_MEM_FRAG_RX - SMC opcode to request next fragment.
 *
 * Register arguments:
 *
 * * w1/w2:  Handle
 * * w3:     Fragment offset.
 * * w4:     Endpoint id ID in [31:16], if client is hypervisor.
 *
 * Return:
 * * w0:             &SMC_FC_FFA_MEM_FRAG_TX
 * * w1/x1-w5/x5:    See &SMC_FC_FFA_MEM_FRAG_TX
 */
#define SMC_FC_FFA_MEM_FRAG_RX SMC_FASTCALL_NR_SHARED_MEMORY(0x7A)

/**
 * SMC_FC_FFA_MEM_FRAG_TX - SMC opcode to transmit next fragment
 *
 * Register arguments:
 *
 * * w1/w2:  Handle
 * * w3:     Fragment length.
 * * w4:     Sender endpoint id ID in [31:16], if client is hypervisor.
 *
 * Return:
 * * w0:             &SMC_FC_FFA_MEM_FRAG_RX or &SMC_FC_FFA_SUCCESS.
 * * w1/x1-w5/x5:    See opcode in w0.
 */
#define SMC_FC_FFA_MEM_FRAG_TX SMC_FASTCALL_NR_SHARED_MEMORY(0x7B)

/**
 * SMC_FC_FFA_CONSOLE_LOG - SMC opcode to log to console
 *
 * Register arguments:
 *
 * * w1:  Count of characters
 * * w2/x2-w7/x7: Packed characters
 *
 * Return:
 * * w0:     &SMC_FC_FFA_SUCCESS
 *
 * or
 *
 * * w0:     &SMC_FC_FFA_ERROR
 * * w2:     Error code (&enum ffa_error)
 * * w3:     Num characters logged (if w2 is RETRY)
 */
#define SMC_FC_FFA_CONSOLE_LOG SMC_FASTCALL_NR_SHARED_MEMORY(0x8A)
#define SMC_FC64_FFA_CONSOLE_LOG SMC_FASTCALL64_NR_SHARED_MEMORY(0x8A)

/* FF-A v1.1 */
/**
 * SMC_FC64_FFA_SECONDARY_EP_REGISTER - SMC opcode to register secondary
 * core entrypoint.
 *
 * Register arguments:
 *
 * * x1:     Entry point address of a secondary execution context
 *
 * Return:
 * * x0:     &SMC_FC64_FFA_SUCCESS
 *
 * or
 *
 * * x0:     &SMC_FC_FFA_ERROR
 * * x2:     %FFA_ERROR_NOT_SUPPORTED Not supported
 *           %FFA_ERROR_INVALID_PARAMETERS Invalid entry point specified
 */
#define SMC_FC64_FFA_SECONDARY_EP_REGISTER SMC_FASTCALL64_NR_SHARED_MEMORY(0x87)

/* Framework messages */
/**
 * FFA_FRAMEWORK_MSG_FLAG - Direct message flag for framework messages.
 *
 * Framework messages have &FFA_FRAMEWORK_MSG_FLAG set in w2.
 */
#define FFA_FRAMEWORK_MSG_FLAG (1U << 31)

/**
 * FFA_FRAMEWORK_MSG_MASK - Mask for the framework message type.
 *
 * Mask the value in w2 against &FFA_FRAMEWORK_MSG_MASK
 * to get the framework message type.
 */
#define FFA_FRAMEWORK_MSG_MASK (0xffU)

/**
 * FFA_FRAMEWORK_MSG_VM_CREATED_REQ - VM creation request.
 *
 * Register arguments:
 *
 * * w0:     &SMC_FC_FFA_MSG_SEND_DIRECT_REQ
 * * w1:     Hypervisor ID in bit[31:16], SP ID in [15:0]
 * * w2:     Message Flags.
 *           bit[31]   : 1 for framework message.
 *           bit[30:8] : Reserved. Must be 0.
 *           bit[7:0]  : &FFA_FRAMEWORK_MSG_VM_CREATED_REQ
 * * w3/w4:  Handle to identify a memory region.
 * * w5:     ID of VM in [15:0], remaining SBZ.
 * * w6-7:   Should be zero.
 */
#define FFA_FRAMEWORK_MSG_VM_CREATED_REQ 4

/**
 * FFA_FRAMEWORK_MSG_VM_CREATED_RESP - VM creation response.
 *
 * Register arguments:
 *
 * * w0:     &SMC_FC_FFA_MSG_SEND_DIRECT_RESP
 * * w1:     SP ID in bit[31:16], hypervisor ID in [15:0]
 * * w2:     Message Flags.
 *           bit[31]   : 1 for framework message.
 *           bit[30:8] : Reserved. Must be 0.
 *           bit[7:0]  : &FFA_FRAMEWORK_MSG_VM_CREATED_RESP
 * * w3:     SP return status code:
 *           %0 in case of success
 *           %FFA_ERROR_INVALID_PARAMETERS
 *           %FFA_ERROR_INTERRUPTED
 *           %FFA_ERROR_DENIED
 *           %FFA_ERROR_RETRY
 * * w4-7:   Should be zero.
 */
#define FFA_FRAMEWORK_MSG_VM_CREATED_RESP 5

/**
 * FFA_FRAMEWORK_MSG_VM_DESTROYED_REQ - VM destruction request.
 *
 * Register arguments:
 *
 * * w0:     &SMC_FC_FFA_MSG_SEND_DIRECT_REQ
 * * w1:     Hypervisor ID in bit[31:16], SP ID in [15:0]
 * * w2:     Message Flags.
 *           bit[31]   : 1 for framework message.
 *           bit[30:8] : Reserved. Must be 0.
 *           bit[7:0]  : &FFA_FRAMEWORK_MSG_VM_DESTROYED_REQ
 * * w3/w4:  Handle to identify a memory region.
 * * w5:     ID of VM in [15:0], remaining SBZ.
 * * w6-7:   Should be zero.
 */
#define FFA_FRAMEWORK_MSG_VM_DESTROYED_REQ 6

/**
 * FFA_FRAMEWORK_MSG_VM_DESTROYED_RESP - VM destruction response.
 *
 * Register arguments:
 *
 * * w0:     &SMC_FC_FFA_MSG_SEND_DIRECT_RESP
 * * w1:     SP ID in bit[31:16], hypervisor ID in [15:0]
 * * w2:     Message Flags.
 *           bit[31]   : 1 for framework message.
 *           bit[30:8] : Reserved. Must be 0.
 *           bit[7:0]  : &FFA_FRAMEWORK_MSG_VM_DESTROYED_RESP
 * * w3:     SP return status code:
 *           %0 in case of success
 *           %FFA_ERROR_INVALID_PARAMETERS
 *           %FFA_ERROR_INTERRUPTED
 *           %FFA_ERROR_DENIED
 *           %FFA_ERROR_RETRY
 * * w4-7:   Should be zero.
 */
#define FFA_FRAMEWORK_MSG_VM_DESTROYED_RESP 7
