/* stateless.cpp — Spec-compliant stateless guest implementation.
 *
 * Spec refs:
 *   stateless_guest.py  §run_stateless_guest
 *   stateless.py        §verify_stateless_new_payload, §compute_new_payload_request_root
 *   stateless_ssz.py    §SszStatelessInput, §SszNewPayloadRequest
 *
 * Wire format: SSZ-encoded SszStatelessInput (per Python spec).
 *
 * NOTE: The Rust reference implementation (stateless-validator-reth) uses bincode
 * for the input wire format instead of SSZ.  Per spec we follow SSZ; if the host
 * switches to bincode this decoder must be updated.
 *
 * EVM execution: Implemented via zilkworm's Blockchain + StateTransition APIs:
 *   1. FlatNodeStore::populate_from_rlp()  ← witness.state (MPT nodes)
 *   2. read_pre_state_from_rlp()           ← witness.state (accounts/storage/code)
 *   3. state.insert_block()                ← witness.headers (ancestor headers)
 *   4. Blockchain::insert_block(block, false) ← execute the block
 *   5. StateTransition::check_root()       ← verify post-state root
 */

#include <z6m/stateless.hpp>
#include <z6m/stateless_types.hpp>
#include <z6m/ssz.hpp>

// zilkworm APIs
#include <zilk_core/core/chain/config.hpp>
#include <zilk_core/core/chain/genesis.hpp>
#include <zilk_core/core/common/bytes.hpp>
#include <zilk_core/core/common/util.hpp>
#include <zilk_core/core/execution/execution.hpp>
#include <zilk_core/core/protocol/blockchain.hpp>
#include <zilk_core/core/rlp/decode.hpp>
#include <zilk_core/core/rlp/encode.hpp>
#include <zilk_core/core/state/in_memory_state.hpp>
#include <zilk_core/core/trie_zz/flat_store.hpp>
#include <zilk_core/core/trie_zz/mpt.hpp>
#include <zilk_core/core/types/block.hpp>
#include <zilk_core/print.hpp>

#include <cstring>
#include <vector>

namespace z6m {

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

    // 3. Witness-backed stateless EVM execution.
    //    Spec ref: stateless.py§verify_stateless_new_payload
    //
    // Wire layout of witness.state (List[ByteList]) from the Python spec:
    //   - Entry 0: RLP-encoded pre-state (accounts / storage / codes) for
    //              read_pre_state_from_rlp().
    //   - Entry 1: RLP-encoded MPT trie nodes for FlatNodeStore::populate_from_rlp().
    //   (Additional entries are ignored; missing entries degrade gracefully.)
    //
    // Wire layout of witness.headers: each ByteSpan is an RLP-encoded BlockHeader
    // for ancestor blocks needed by Blockchain (parent + any EIP-4788 ring-buffer).

    bool successful_validation = false;

    const auto& wit = si.witness;
    const SszExecutionPayload& ep = si.new_payload_request.execution_payload;

    // Resolve chain config from chain_id provided in the input.
    // SmallMap::find() returns const T* (nullptr if not found).
    const silkworm::ChainConfig* chain_cfg = nullptr;
    {
        const silkworm::ChainConfig* const* found =
            silkworm::kKnownChainConfigs.find(si.chain_config.chain_id);
        chain_cfg = found ? *found : &silkworm::kMainnetConfig;
    }

    // Decode pre-state (accounts + storage + codes) from witness.state[0].
    // Decode trie nodes (MPT proof) from witness.state[1].
    silkworm::InMemoryState state;
    silkworm::mpt::FlatNodeStore node_store;

    if (wit.state.size() >= 1 && wit.state[0].len > 0) {
        silkworm::ByteView pre_state_rlp{wit.state[0].ptr, wit.state[0].len};
        state = silkworm::read_pre_state_from_rlp(pre_state_rlp);
    }
    if (wit.state.size() >= 2 && wit.state[1].len > 0) {
        silkworm::ByteView trie_rlp{wit.state[1].ptr, wit.state[1].len};
        node_store.populate_from_rlp(trie_rlp);
    }

    // Load contract bytecode from witness.codes.
    // Layout: each ByteSpan is one contract's bytecode (raw bytes, no RLP wrapping).
    for (const ByteSpan& code_span : wit.codes) {
        if (code_span.len == 0) continue;
        silkworm::ByteView code{code_span.ptr, code_span.len};
        // Compute the code hash (keccak256) and register with the state.
        silkworm::Bytes code_bytes(code.begin(), code.end());
        auto code_hash = std::bit_cast<evmc::bytes32>(ethash_keccak256(code.data(), code.size()).bytes);
        // Use a placeholder address — InMemoryState keys code by hash, not address.
        state.update_account_code(evmc::address{}, code_hash, code_bytes);
    }

    // Load ancestor block headers from witness.headers so Blockchain can resolve
    // parent_hash references and the EIP-4788 beacon root ring buffer.
    for (const ByteSpan& hdr_span : wit.headers) {
        if (hdr_span.len == 0) continue;
        silkworm::ByteView hdr_rlp{hdr_span.ptr, hdr_span.len};
        silkworm::Block anc;
        if (silkworm::rlp::decode(hdr_rlp, anc.header)) {
            auto hash = anc.header.hash();
            state.insert_block(anc, hash);
        }
    }

    // Build the parent (genesis) block from witness.headers[0] if available,
    // otherwise synthesise a minimal genesis from the execution payload's parent_hash.
    silkworm::Block genesis_block;
    if (!wit.headers.empty() && wit.headers[0].len > 0) {
        silkworm::ByteView hdr_rlp{wit.headers[0].ptr, wit.headers[0].len};
        silkworm::rlp::decode(hdr_rlp, genesis_block.header);
    } else {
        // Minimal fallback: set parent_hash so Blockchain can anchor the chain.
        std::memcpy(genesis_block.header.parent_hash.bytes, ep.parent_hash, 32);
        genesis_block.header.number = ep.block_number > 0 ? ep.block_number - 1 : 0;
    }

    // Construct the execution block from the SSZ ExecutionPayload.
    silkworm::Block block;
    std::memcpy(block.header.parent_hash.bytes,     ep.parent_hash,    32);
    std::memcpy(block.header.beneficiary.bytes,     ep.fee_recipient,  20);
    std::memcpy(block.header.state_root.bytes,      ep.state_root,     32);
    std::memcpy(block.header.receipts_root.bytes,   ep.receipts_root,  32);
    std::memcpy(block.header.logs_bloom.data(),     ep.logs_bloom,    256);
    std::memcpy(block.header.prev_randao.bytes,     ep.prev_randao,    32);
    block.header.number        = ep.block_number;
    block.header.gas_limit     = ep.gas_limit;
    block.header.gas_used      = ep.gas_used;
    block.header.timestamp     = ep.timestamp;
    block.header.extra_data    = silkworm::Bytes(ep.extra_data.ptr,
                                                 ep.extra_data.ptr + ep.extra_data.len);
    // base_fee_per_gas: 32-byte big-endian
    block.header.base_fee_per_gas =
        intx::be::load<intx::uint256>(ep.base_fee_per_gas);
    // Note: BlockHeader has no block_hash field — the hash is computed on-demand
    // by BlockHeader::hash() from the RLP encoding.
    block.header.blob_gas_used   = ep.blob_gas_used;
    block.header.excess_blob_gas = ep.excess_blob_gas;

    // Decode transactions: each ByteSpan is an opaque transaction blob.
    block.transactions.reserve(ep.transactions.size());
    for (const ByteSpan& tx_span : ep.transactions) {
        silkworm::Transaction tx;
        silkworm::ByteView tx_view{tx_span.ptr, tx_span.len};
        if (silkworm::rlp::decode(tx_view, tx)) {
            block.transactions.push_back(std::move(tx));
        }
    }

    // Decode withdrawals.
    block.withdrawals.emplace(); // activate optional
    for (const SszWithdrawal& sw : ep.withdrawals) {
        silkworm::Withdrawal w;
        w.index           = sw.index;
        w.validator_index = sw.validator_index;
        std::memcpy(w.address.bytes, sw.address, 20);
        w.amount          = sw.amount;
        block.withdrawals->push_back(w);
    }

    // Execute via Blockchain. check_state_root=false — we verify it ourselves below.
    silkworm::protocol::Blockchain blockchain{state, *chain_cfg, genesis_block};
    // ValidationResult is in silkworm:: namespace (not silkworm::protocol::).
    silkworm::ValidationResult exec_result =
        blockchain.insert_block(block, /*check_state_root=*/false);

    if (exec_result == silkworm::ValidationResult::kOk) {
        // Verify the post-state root.
        // Inline StateTransition::check_root() logic: rebuild the MPT delta from
        // account/storage changes and compare the resulting root to block.header.state_root.
        node_store.populate_from_rlp(
            (wit.state.size() >= 2 && wit.state[1].len > 0)
                ? silkworm::ByteView{wit.state[1].ptr, wit.state[1].len}
                : silkworm::ByteView{});

        const auto& acc_changes   = state.account_changes().at(block.header.number);
        const auto& stor_changes  = state.storage_changes().at(block.header.number);

        std::vector<silkworm::mpt::TrieNodeFlat> acc_updates;
        silkworm::Bytes val_rlp;
        val_rlp.reserve(33);

        for (auto& [addr, acc_opt] : acc_changes) {
            const silkworm::Account& acc = acc_opt.has_value() ? acc_opt.value() : silkworm::Account{};
            evmc::bytes32 storage_root = acc.storage_root_;

            auto stor_it = stor_changes.find(addr);
            if (stor_it != stor_changes.end()) {
                std::vector<silkworm::mpt::TrieNodeFlat> stor_updates;
                for (auto& [key, val] : stor_it->second) {
                    auto cur = state.read_storage(addr, key);
                    if (cur == val) continue;
                    auto zv = silkworm::zeroless_view(cur.bytes);
                    val_rlp.clear();
                    silkworm::rlp::encode(val_rlp, zv);
                    stor_updates.emplace_back(silkworm::mpt::TrieNodeFlat{
                        silkworm::keccak_bytes32(key), val_rlp});
                }
                if (!stor_updates.empty()) {
                    if (silkworm::mpt::is_zero_quick(acc.storage_root_))
                        storage_root = silkworm::kEmptyRoot;
                    silkworm::mpt::GridMPT<true> stor_trie{node_store, storage_root};
                    std::sort(stor_updates.begin(), stor_updates.end());
                    storage_root = stor_trie.calc_root_from_updates(stor_updates);
                }
            }

            auto cur_acc_opt = state.read_account(addr);
            if (!cur_acc_opt) continue;
            if (acc == *cur_acc_opt && storage_root == acc.storage_root_) continue;

            auto acc_rlp  = cur_acc_opt->rlp(storage_root);
            auto addr_hash = silkworm::keccak_bytes(addr.bytes);
            acc_updates.emplace_back(addr_hash, acc_rlp);
        }

        std::sort(acc_updates.begin(), acc_updates.end());
        auto parent_hdr = state.read_header(block.header.number - 1, block.header.parent_hash);
        evmc::bytes32 prev_root = parent_hdr ? parent_hdr->state_root : evmc::bytes32{};
        silkworm::mpt::GridMPT<false> acc_trie{node_store, prev_root};
        evmc::bytes32 new_root = acc_trie.calc_root_from_updates(acc_updates);
        successful_validation = (new_root == block.header.state_root);
    }

    StatelessValidatorOutput result{};
    std::memcpy(result.new_payload_request_root, npr_root, 32);
    result.successful_validation = successful_validation;
    return result;
}

} // namespace z6m
