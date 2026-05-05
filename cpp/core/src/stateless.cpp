/* stateless.cpp — Spec-compliant stateless guest implementation.
 *
 * Spec refs:
 *   stateless_guest.py  §run_stateless_guest
 *   stateless.py        §verify_stateless_new_payload, §compute_new_payload_request_root
 *   stateless_ssz.py    §SszStatelessInput, §SszNewPayloadRequest, §SszStatelessValidationResult
 *
 * Wire format: SSZ-encoded SszStatelessInput (per Python spec).
 *
 * NOTE: The Rust reference implementation (stateless-validator-reth) uses bincode
 * for the input wire format instead of SSZ.  Per task instructions we follow the
 * spec (SSZ) and flag this discrepancy here.  If the host sends bincode the C++
 * guest must be updated to match — but the spec is unambiguous about SSZ.
 *
 * EVM re-execution stub: Full witness-based stateless EVM execution requires a
 * Merkle-Patricia-Trie implementation backed by the witness nodes.  Zilkworm's
 * StateTransition does not expose a witness-trie interface, so the validation
 * step is stubbed and returns successful_validation = false.
 * TODO: Implement witness-trie-backed execution once zilkworm exposes the API.
 */

#include <z6m/stateless.hpp>
#include <z6m/stateless_types.hpp>
#include <z6m/ssz.hpp>

#include <cstring>
#include <vector>

namespace z6m {

// ══════════════════════════════════════════════════════════════════════════════
// SSZ Deserialization
// ══════════════════════════════════════════════════════════════════════════════



// Decode SszWithdrawal from fixed-size bytes
static SszWithdrawal decode_withdrawal(ByteSpan s) {
    SszWithdrawal w{};
    w.index            = read_u64le(s.ptr);      s = s.from(8);
    w.validator_index  = read_u64le(s.ptr);      s = s.from(8);
    std::memcpy(w.address, s.ptr, 20);           s = s.from(20);
    w.amount           = read_u64le(s.ptr);
    return w;
}

static SszDepositRequest decode_deposit_request(ByteSpan s) {
    SszDepositRequest d{};
    std::memcpy(d.pubkey, s.ptr, 48);                      s = s.from(48);
    std::memcpy(d.withdrawal_credentials, s.ptr, 32);      s = s.from(32);
    d.amount = read_u64le(s.ptr);                          s = s.from(8);
    std::memcpy(d.signature, s.ptr, 96);                   s = s.from(96);
    d.index  = read_u64le(s.ptr);
    return d;
}

static SszWithdrawalRequest decode_withdrawal_request(ByteSpan s) {
    SszWithdrawalRequest w{};
    std::memcpy(w.source_address,   s.ptr, 20);  s = s.from(20);
    std::memcpy(w.validator_pubkey, s.ptr, 48);  s = s.from(48);
    w.amount = read_u64le(s.ptr);
    return w;
}

static SszConsolidationRequest decode_consolidation_request(ByteSpan s) {
    SszConsolidationRequest c{};
    std::memcpy(c.source_address, s.ptr, 20);  s = s.from(20);
    std::memcpy(c.source_pubkey,  s.ptr, 48);  s = s.from(48);
    std::memcpy(c.target_pubkey,  s.ptr, 48);
    return c;
}

// Decode an SSZ list of fixed-size items.
// Returns spans into the parent data; caller decodes items from each span.
template<typename T, typename DecFn>
static std::vector<T> decode_fixed_list(ByteSpan data, size_t item_size, DecFn decode_fn) {
    std::vector<T> out;
    size_t n = data.len / item_size;
    out.reserve(n);
    for (size_t i = 0; i < n; ++i)
        out.push_back(decode_fn(data.slice(i * item_size, item_size)));
    return out;
}

// SSZ container fixed-part size for SszExecutionRequests:
// 3 variable-length fields → 3 × 4-byte offsets = 12 bytes fixed part
static constexpr size_t EXEC_REQ_FIXED = 12;

static SszExecutionRequests decode_execution_requests(ByteSpan s) {
    SszExecutionRequests er{};
    if (s.len < EXEC_REQ_FIXED) return er;
    uint32_t off0 = read_u32le(s.ptr);
    uint32_t off1 = read_u32le(s.ptr + 4);
    uint32_t off2 = read_u32le(s.ptr + 8);
    uint32_t end  = static_cast<uint32_t>(s.len);

    ByteSpan deposits_data     = s.slice(off0, off1 - off0);
    ByteSpan withdrawals_data  = s.slice(off1, off2 - off1);
    ByteSpan consols_data      = s.slice(off2, end  - off2);

    er.deposits      = decode_fixed_list<SszDepositRequest>(deposits_data,
                           SSZ_DEPOSIT_REQUEST_FIXED_SIZE, decode_deposit_request);
    er.withdrawals   = decode_fixed_list<SszWithdrawalRequest>(withdrawals_data,
                           SSZ_WITHDRAWAL_REQUEST_FIXED_SIZE, decode_withdrawal_request);
    er.consolidations= decode_fixed_list<SszConsolidationRequest>(consols_data,
                           SSZ_CONSOLIDATION_REQUEST_FIXED_SIZE, decode_consolidation_request);
    return er;
}

// Decode a list of ByteList items (each prefixed by a 4-byte SSZ offset in an offset table).
// `data` is the raw list bytes (offset table + variable data).
static std::vector<ByteSpan> decode_bytelist_list(ByteSpan data) {
    std::vector<ByteSpan> out;
    if (data.len < 4) return out;
    uint32_t first_offset = read_u32le(data.ptr);
    if (first_offset % 4 != 0 || first_offset > data.len) return out;
    size_t n = first_offset / 4;
    out.reserve(n);
    for (size_t i = 0; i < n; ++i) {
        uint32_t this_off = read_u32le(data.ptr + i * 4);
        uint32_t next_off = (i + 1 < n) ? read_u32le(data.ptr + (i + 1) * 4)
                                         : static_cast<uint32_t>(data.len);
        out.push_back(data.slice(this_off, next_off - this_off));
    }
    return out;
}


// SszExecutionPayload fixed-part size:
// parent_hash(32) + fee_recipient(20) + state_root(32) + receipts_root(32) +
// logs_bloom(256) + prev_randao(32) + block_number(8) + gas_limit(8) +
// gas_used(8) + timestamp(8) + extra_data_offset(4) + base_fee_per_gas(32) +
// block_hash(32) + transactions_offset(4) + withdrawals_offset(4) +
// blob_gas_used(8) + excess_blob_gas(8) + block_access_list_offset(4)
// = 32+20+32+32+256+32+8+8+8+8+4+32+32+4+4+8+8+4 = 542
static constexpr size_t PAYLOAD_FIXED = 542;

static SszExecutionPayload decode_execution_payload(ByteSpan s) {
    SszExecutionPayload p{};
    if (s.len < PAYLOAD_FIXED) return p;
    size_t cursor = 0;
    auto advance = [&](size_t n) -> const uint8_t* {
        const uint8_t* r = s.ptr + cursor; cursor += n; return r;
    };
    std::memcpy(p.parent_hash,     advance(32), 32);
    std::memcpy(p.fee_recipient,   advance(20), 20);
    std::memcpy(p.state_root,      advance(32), 32);
    std::memcpy(p.receipts_root,   advance(32), 32);
    std::memcpy(p.logs_bloom,      advance(256), 256);
    std::memcpy(p.prev_randao,     advance(32), 32);
    p.block_number  = read_u64le(advance(8));
    p.gas_limit     = read_u64le(advance(8));
    p.gas_used      = read_u64le(advance(8));
    p.timestamp     = read_u64le(advance(8));
    uint32_t off_extra_data   = read_u32le(advance(4));
    std::memcpy(p.base_fee_per_gas, advance(32), 32);
    std::memcpy(p.block_hash,  advance(32), 32);
    uint32_t off_transactions = read_u32le(advance(4));
    uint32_t off_withdrawals  = read_u32le(advance(4));
    p.blob_gas_used  = read_u64le(advance(8));
    p.excess_blob_gas= read_u64le(advance(8));
    uint32_t off_bal          = read_u32le(advance(4));
    // cursor == PAYLOAD_FIXED now

    uint32_t total = static_cast<uint32_t>(s.len);
    p.extra_data        = s.slice(off_extra_data,    off_transactions - off_extra_data);
    p.transactions      = decode_bytelist_list(s.slice(off_transactions, off_withdrawals - off_transactions));

    // withdrawals: list of fixed-size SszWithdrawal
    ByteSpan wdl_data = s.slice(off_withdrawals, off_bal - off_withdrawals);
    p.withdrawals = decode_fixed_list<SszWithdrawal>(wdl_data,
                        SSZ_WITHDRAWAL_FIXED_SIZE, decode_withdrawal);
    p.block_access_list = s.slice(off_bal, total - off_bal);
    return p;
}

// SszNewPayloadRequest fixed-part:
// execution_payload_offset(4) + versioned_hashes_offset(4) +
// parent_beacon_block_root(32) + execution_requests_offset(4) = 44
static constexpr size_t NPR_FIXED = 44;

static SszNewPayloadRequest decode_new_payload_request(ByteSpan s) {
    SszNewPayloadRequest r{};
    if (s.len < NPR_FIXED) return r;
    uint32_t off_payload = read_u32le(s.ptr);
    uint32_t off_hashes  = read_u32le(s.ptr + 4);
    std::memcpy(r.parent_beacon_block_root, s.ptr + 8, 32);
    uint32_t off_er      = read_u32le(s.ptr + 40);
    uint32_t total       = static_cast<uint32_t>(s.len);

    r.execution_payload  = decode_execution_payload(s.slice(off_payload, off_hashes - off_payload));
    // versioned_hashes: List[Bytes32, MAX_BLOB_COMMITMENTS_PER_BLOCK]
    // Each Bytes32 is fixed-size, so this list has no offset table — it's a flat array.
    ByteSpan vh_data = s.slice(off_hashes, off_er - off_hashes);
    size_t n_vh = vh_data.len / 32;
    r.versioned_hashes.reserve(n_vh);
    for (size_t i = 0; i < n_vh; ++i) {
        std::array<uint8_t,32> h;
        std::memcpy(h.data(), vh_data.ptr + i * 32, 32);
        r.versioned_hashes.push_back(h);
    }
    r.execution_requests = decode_execution_requests(s.slice(off_er, total - off_er));
    return r;
}

// SszExecutionWitness fixed-part: 3 variable fields → 12 bytes
static constexpr size_t WITNESS_FIXED = 12;

static SszExecutionWitness decode_witness(ByteSpan s) {
    SszExecutionWitness w{};
    if (s.len < WITNESS_FIXED) return w;
    uint32_t off_state   = read_u32le(s.ptr);
    uint32_t off_codes   = read_u32le(s.ptr + 4);
    uint32_t off_headers = read_u32le(s.ptr + 8);
    uint32_t total       = static_cast<uint32_t>(s.len);

    w.state   = decode_bytelist_list(s.slice(off_state,   off_codes   - off_state));
    w.codes   = decode_bytelist_list(s.slice(off_codes,   off_headers - off_codes));
    w.headers = decode_bytelist_list(s.slice(off_headers, total       - off_headers));
    return w;
}

// SszStatelessInput fixed-part: 4 variable fields → 16 bytes
static constexpr size_t STATELESS_INPUT_FIXED = 16;

static SszStatelessInput decode_stateless_input(ByteSpan s) {
    SszStatelessInput si{};
    if (s.len < STATELESS_INPUT_FIXED) return si;
    uint32_t off_npr    = read_u32le(s.ptr);
    uint32_t off_wit    = read_u32le(s.ptr + 4);
    uint32_t off_cc     = read_u32le(s.ptr + 8);
    uint32_t off_pk     = read_u32le(s.ptr + 12);
    uint32_t total      = static_cast<uint32_t>(s.len);

    si.new_payload_request = decode_new_payload_request(s.slice(off_npr, off_wit - off_npr));
    si.witness             = decode_witness(s.slice(off_wit, off_cc - off_wit));
    // chain_config is fixed-size: just chain_id (uint64)
    si.chain_config.chain_id = read_u64le(s.ptr + off_cc);
    // public_keys: List[ByteList[65], MAX_PUBLIC_KEYS]
    si.public_keys = decode_bytelist_list(s.slice(off_pk, total - off_pk));
    return si;
}

// ══════════════════════════════════════════════════════════════════════════════
// SSZ hash_tree_root for SszNewPayloadRequest
// Spec ref: stateless.py§compute_new_payload_request_root → ssz_npr.hash_tree_root()
// ══════════════════════════════════════════════════════════════════════════════

static void htr_withdrawal(uint8_t out[32], const SszWithdrawal& w) {
    // Fixed container: 4 fields → merkleize([htr(f) for f in fields])
    uint8_t fields[4][32] = {};
    htr_uint64(fields[0], w.index);
    htr_uint64(fields[1], w.validator_index);
    htr_byte_vector(fields[2], w.address, 20);
    htr_uint64(fields[3], w.amount);
    htr_container(out, fields, 4);
}

static void htr_deposit_request(uint8_t out[32], const SszDepositRequest& d) {
    uint8_t fields[5][32] = {};
    htr_byte_vector(fields[0], d.pubkey, 48);
    htr_uint256(fields[1], d.withdrawal_credentials);
    htr_uint64 (fields[2], d.amount);
    htr_byte_vector(fields[3], d.signature, 96);
    htr_uint64 (fields[4], d.index);
    htr_container(out, fields, 5);
}

static void htr_withdrawal_request(uint8_t out[32], const SszWithdrawalRequest& w) {
    uint8_t fields[3][32] = {};
    htr_byte_vector(fields[0], w.source_address, 20);
    htr_byte_vector(fields[1], w.validator_pubkey, 48);
    htr_uint64 (fields[2], w.amount);
    htr_container(out, fields, 3);
}

static void htr_consolidation_request(uint8_t out[32], const SszConsolidationRequest& c) {
    uint8_t fields[3][32] = {};
    htr_byte_vector(fields[0], c.source_address, 20);
    htr_byte_vector(fields[1], c.source_pubkey, 48);
    htr_byte_vector(fields[2], c.target_pubkey, 48);
    htr_container(out, fields, 3);
}

// hash_tree_root of a list of fixed-size SSZ containers.
// chunks = [htr(item) for item in items], merkleize(chunks, limit) + mix_in_length
template<typename T, typename HtrFn>
static void htr_container_list(uint8_t out[32], const std::vector<T>& items,
                                 size_t limit, HtrFn item_htr) {
    size_t n = items.size();
    // Build chunk array on heap via vector
    std::vector<uint8_t> chunks(n * 32, 0);
    for (size_t i = 0; i < n; ++i)
        item_htr(chunks.data() + i * 32, items[i]);
    uint8_t root[32];
    merkleize(root, chunks.data(), n, limit);
    mix_in_length(out, root, n);
}

static void htr_execution_requests(uint8_t out[32], const SszExecutionRequests& er) {
    uint8_t fields[3][32] = {};
    htr_container_list(fields[0], er.deposits,       MAX_DEPOSIT_REQUESTS_PER_PAYLOAD,       htr_deposit_request);
    htr_container_list(fields[1], er.withdrawals,    MAX_WITHDRAWAL_REQUESTS_PER_PAYLOAD,    htr_withdrawal_request);
    htr_container_list(fields[2], er.consolidations, MAX_CONSOLIDATION_REQUESTS_PER_PAYLOAD, htr_consolidation_request);
    htr_container(out, fields, 3);
}

static void htr_execution_payload(uint8_t out[32], const SszExecutionPayload& p) {
    // 17 fields per spec
    uint8_t fields[17][32] = {};

    htr_uint256(fields[0],  p.parent_hash);
    htr_byte_vector(fields[1], p.fee_recipient, 20);
    htr_uint256(fields[2],  p.state_root);
    htr_uint256(fields[3],  p.receipts_root);
    htr_byte_vector(fields[4], p.logs_bloom, 256);
    htr_uint256(fields[5],  p.prev_randao);
    htr_uint64 (fields[6],  p.block_number);
    htr_uint64 (fields[7],  p.gas_limit);
    htr_uint64 (fields[8],  p.gas_used);
    htr_uint64 (fields[9],  p.timestamp);
    // extra_data: ByteList[MAX_EXTRA_DATA_BYTES]
    htr_byte_list(fields[10], p.extra_data.ptr, p.extra_data.len, MAX_EXTRA_DATA_BYTES);
    htr_uint256(fields[11], p.base_fee_per_gas);
    htr_uint256(fields[12], p.block_hash);

    // transactions: List[ByteList[MAX_BYTES_PER_TRANSACTION], MAX_TRANSACTIONS_PER_PAYLOAD]
    {
        size_t ntx = p.transactions.size();
        std::vector<uint8_t> tx_chunks(ntx * 32, 0);
        for (size_t i = 0; i < ntx; ++i)
            htr_byte_list(tx_chunks.data() + i * 32,
                          p.transactions[i].ptr, p.transactions[i].len,
                          MAX_BYTES_PER_TRANSACTION);
        uint8_t tx_root[32];
        merkleize(tx_root, tx_chunks.data(), ntx, MAX_TRANSACTIONS_PER_PAYLOAD);
        mix_in_length(fields[13], tx_root, ntx);
    }

    // withdrawals: List[SszWithdrawal, MAX_WITHDRAWALS_PER_PAYLOAD]
    htr_container_list(fields[14], p.withdrawals, MAX_WITHDRAWALS_PER_PAYLOAD, htr_withdrawal);

    htr_uint64 (fields[15], p.blob_gas_used);
    htr_uint64 (fields[16], p.excess_blob_gas);
    // Note: block_access_list is field 17 (index 17) per spec.
    // SszExecutionPayload has 18 fields in the Amsterdam fork spec.

    // Expand to 18 fields for block_access_list
    uint8_t fields18[18][32] = {};
    for (int i = 0; i < 17; ++i) std::memcpy(fields18[i], fields[i], 32);
    htr_byte_list(fields18[17], p.block_access_list.ptr, p.block_access_list.len,
                  MAX_BLOCK_ACCESS_LIST_BYTES);
    htr_container(out, fields18, 18);
}

static void htr_new_payload_request(uint8_t out[32], const SszNewPayloadRequest& r) {
    // 4 fields: execution_payload, versioned_hashes, parent_beacon_block_root, execution_requests
    uint8_t fields[4][32] = {};

    htr_execution_payload(fields[0], r.execution_payload);

    // versioned_hashes: List[Bytes32, MAX_BLOB_COMMITMENTS_PER_BLOCK]
    {
        size_t n = r.versioned_hashes.size();
        std::vector<uint8_t> vh_chunks(n * 32, 0);
        for (size_t i = 0; i < n; ++i)
            std::memcpy(vh_chunks.data() + i * 32, r.versioned_hashes[i].data(), 32);
        uint8_t vh_root[32];
        merkleize(vh_root, vh_chunks.data(), n, MAX_BLOB_COMMITMENTS_PER_BLOCK);
        mix_in_length(fields[1], vh_root, n);
    }

    htr_uint256(fields[2], r.parent_beacon_block_root);
    htr_execution_requests(fields[3], r.execution_requests);
    htr_container(out, fields, 4);
}


StatelessValidatorOutput run_stateless_guest(const uint8_t* data, size_t len) {
    ByteSpan input{data, len};

    // 1. Deserialise SszStatelessInput.
    //    Spec ref: stateless_guest.py§deserialize_stateless_input
    SszStatelessInput si = decode_stateless_input(input);

    // 2. Compute hash_tree_root of SszNewPayloadRequest.
    //    Spec ref: stateless.py§compute_new_payload_request_root
    uint8_t npr_root[32];
    htr_new_payload_request(npr_root, si.new_payload_request);

    // 3. Attempt stateless EVM execution.
    //    Spec ref: stateless.py§verify_stateless_new_payload
    //
    // TODO: Implement full witness-backed stateless EVM execution.
    //   Requires rebuilding the Merkle-Patricia-Trie from si.witness.state,
    //   constructing the account/storage state, and re-executing the block.
    //   Zilkworm's StateTransition does not expose a witness-trie interface.
    //   Until that is available, we conservatively report failure (the
    //   hash_tree_root commitment is still correct, which is the critical
    //   public output).
    //
    // SPEC vs RUST discrepancy: The Rust guest uses stateless_validation_with_trie()
    // which performs full re-execution.  When zilkworm gains witness-trie support,
    // replace the stub below with that call.
    const bool successful_validation = false; // STUB — see TODO above

    StatelessValidatorOutput result{};
    std::memcpy(result.new_payload_request_root, npr_root, 32);
    result.successful_validation = successful_validation;
    return result;
}

} // namespace z6m
