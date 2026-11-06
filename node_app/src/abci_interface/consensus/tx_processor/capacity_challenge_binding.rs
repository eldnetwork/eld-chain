use eld_common::{
    address::Address,
    capacity_challenge::{compute_challenge_id, select_challenge_chunk_indices},
    capacity_merkle_root::CapacityMerkleRoot,
    challenge_id::ChallengeId,
    validator::EpochRecord,
};

#[cfg(test)]
use eld_common::coin::Coin;

/// On-chain inputs resolved by binding gates (sender / challenged set / challenge_id).
#[derive(Debug)]
pub(crate) struct VerifiedProofBinding {
    pub challenged_provider_id: Address,
    pub validator_address: Address,
    pub expected_merkle_root: CapacityMerkleRoot,
    pub expected_indices: Vec<usize>,
    #[allow(dead_code)] // asserted in unit tests; kept for bind completeness
    pub expected_challenge_id: ChallengeId,
}

/// Proof epoch is `block_height / BLOCKS_PER_EPOCH`, matching challenge issuance in `end_block`.
///
/// Accept the current epoch or the immediately previous one. Older and future epochs are rejected.
pub(crate) fn proof_epoch_in_window(
    block_height: i64,
    current_epoch: i64,
    blocks_per_epoch: i64,
) -> Result<i64, String> {
    if block_height <= 0 {
        return Err("VerifiedProof rejected: block_height must be positive".to_string());
    }
    if current_epoch < 0 {
        return Err("VerifiedProof rejected: current epoch is invalid".to_string());
    }
    let proof_epoch = block_height / blocks_per_epoch;
    let in_window =
        proof_epoch == current_epoch || (current_epoch > 0 && proof_epoch == current_epoch - 1);
    if in_window {
        Ok(proof_epoch)
    } else if proof_epoch > current_epoch {
        Err(format!(
            "VerifiedProof rejected: proof epoch {proof_epoch} is after current epoch {current_epoch}"
        ))
    } else {
        Err(format!(
            "VerifiedProof rejected: proof epoch {proof_epoch} is older than the previous epoch (current {current_epoch})"
        ))
    }
}

/// Window check, then gates 1–4 against that epoch's [`EpochRecord`].
///
/// `epoch_record` is `None` when the snapshot was not stored. Pure / sync so unit tests can
/// cover rejection without spinning ABCI.
pub(crate) fn bind_verified_proof(
    epoch_record: Option<&EpochRecord>,
    current_epoch: i64,
    sender: Address,
    provider: Address,
    challenge_id: &str,
    block_height: i64,
    protocol: &eld_common::protocol_constants::ProtocolConstants,
) -> Result<VerifiedProofBinding, String> {
    let proof_epoch =
        proof_epoch_in_window(block_height, current_epoch, protocol.blocks_per_epoch)?;
    let epoch_record = epoch_record.ok_or_else(|| {
        format!("VerifiedProof rejected: no epoch record for epoch {proof_epoch}")
    })?;
    validate_verified_proof_binding(
        epoch_record,
        sender,
        provider,
        challenge_id,
        block_height,
        protocol.blocks_per_epoch,
        protocol.chunks_per_challenge,
    )
}

/// Gates 1–4 against one epoch snapshot: sender, challenged set, merkle root, chunk count.
fn validate_verified_proof_binding(
    epoch_record: &EpochRecord,
    sender: Address,
    provider: Address,
    challenge_id: &str,
    block_height: i64,
    blocks_per_epoch: i64,
    chunks_per_challenge: usize,
) -> Result<VerifiedProofBinding, String> {
    let proof_epoch = block_height / blocks_per_epoch;
    if epoch_record.epoch != proof_epoch {
        return Err(format!(
            "VerifiedProof rejected: epoch record {} does not match proof epoch {proof_epoch}",
            epoch_record.epoch
        ));
    }

    // Gate 1: rewards only from the epoch's active capacity validator.
    let active_sv = epoch_record
        .active_capacity_validator
        .as_ref()
        .ok_or_else(|| {
            format!("VerifiedProof rejected: no active_capacity_validator for epoch {proof_epoch}")
        })?;
    if active_sv.epoch != proof_epoch {
        return Err(format!(
            "VerifiedProof rejected: active capacity validator epoch {} does not match proof epoch {proof_epoch}",
            active_sv.epoch
        ));
    }
    if sender != active_sv.validator_address {
        return Err(format!(
            "VerifiedProof rejected: sender {sender} is not active_capacity_validator {}",
            active_sv.validator_address
        ));
    }

    // Gate 2: capacity_provider must be one of the providers selected for this epoch.
    if !active_sv.challenged_providers.contains(&provider) {
        return Err(format!(
            "VerifiedProof rejected: capacity_provider {provider} is not in challenged_providers"
        ));
    }
    let challenged_provider_id = provider;

    // Gate 3: chunk_count + merkle_root frozen on the epoch record, not the live set.
    let provider_info = epoch_record
        .challenged_capacity_validators
        .iter()
        .find(|p| p.address == provider)
        .ok_or_else(|| {
            format!(
                "VerifiedProof rejected: capacity_provider {provider} not in epoch {proof_epoch} challenged_capacity_validators"
            )
        })?;
    let chunk_count = match provider_info.chunk_count {
        Some(c) if c > 0 => c,
        _ => {
            return Err(format!(
                "VerifiedProof rejected: capacity_provider {provider} has no chunk_count"
            ));
        }
    };
    let expected_merkle_root =
        CapacityMerkleRoot::new(provider_info.merkle_root.ok_or_else(|| {
            format!("VerifiedProof rejected: capacity_provider {provider} has no merkle_root")
        })?);
    let validator_address = active_sv.validator_address;

    // Gate 4: recompute challenge_id from on-chain inputs; reject fabricated IDs.
    let expected_indices = select_challenge_chunk_indices(
        active_sv.epoch,
        &challenged_provider_id,
        block_height,
        &validator_address,
        chunk_count,
        chunks_per_challenge,
    );
    let parsed_challenge_id = ChallengeId::parse_hex(challenge_id)
        .map_err(|e| format!("VerifiedProof rejected: invalid challenge_id hex: {e}"))?;
    let expected_challenge_id = compute_challenge_id(
        &validator_address,
        &challenged_provider_id,
        block_height,
        &expected_indices,
    );
    if parsed_challenge_id != expected_challenge_id {
        return Err(format!(
            "VerifiedProof rejected: challenge_id mismatch (got {challenge_id}, expected {})",
            expected_challenge_id.to_hex()
        ));
    }

    Ok(VerifiedProofBinding {
        challenged_provider_id,
        validator_address,
        expected_merkle_root,
        expected_indices,
        expected_challenge_id,
    })
}

/// Increments the provider's failed-proof count. At [`FAILED_PROOFS_BEFORE_SLASH`], burns
/// [`CapacityValidatorInfo::stake`](eld_common::validator::CapacityValidatorInfo::stake) and resets the count.
#[cfg(test)]
pub(crate) fn record_failed_capacity_proof(
    envelope: &mut crate::app_state::AppStateEnvelope,
    provider: Address,
    failed_proofs_before_slash: u32,
) -> (u32, bool) {
    let count = envelope.failed_proof_counts.entry(provider).or_insert(0);
    *count = count.saturating_add(1);
    if *count >= failed_proofs_before_slash {
        for cv in envelope.capacity_validators.iter_mut() {
            if cv.address == provider {
                cv.stake = Coin::zero();
                break;
            }
        }
        envelope.failed_proof_counts.insert(provider, 0);
        return (0, true);
    }
    let recorded = *count;
    (recorded, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use eld_common::address::Address;
    use eld_common::public_key::PublicKey;
    use eld_common::validator::{ActiveCapacityValidator, CapacityValidatorInfo};

    fn addr(hex: &str) -> Address {
        Address::parse_hex_str(hex).expect("test address")
    }

    fn test_public_key(seed: u8) -> PublicKey {
        PublicKey::from(SigningKey::from_bytes(&[seed; 32]).verifying_key())
    }

    const VALIDATOR: &str = "0x1111111111111111111111111111111111111111";
    const PROVIDER: &str = "0x2222222222222222222222222222222222222222";
    const OTHER: &str = "0x3333333333333333333333333333333333333333";
    const EPOCH: i64 = 3;
    const BLOCKS_PER_EPOCH: i64 = 20;
    const CHUNKS_PER_CHALLENGE: usize = 10;
    const FAILED_PROOFS_BEFORE_SLASH: u32 = 3;
    /// Epoch-start height. Challenges are issued at `epoch * BLOCKS_PER_EPOCH`.
    const BLOCK_HEIGHT: i64 = EPOCH * BLOCKS_PER_EPOCH;
    const CHUNK_COUNT: u32 = 50;

    fn epoch_record(
        epoch: i64,
        challenged: Vec<&str>,
        providers: Vec<CapacityValidatorInfo>,
        with_active: bool,
    ) -> EpochRecord {
        EpochRecord {
            epoch,
            start_block: epoch * BLOCKS_PER_EPOCH,
            active_validators: vec![],
            active_capacity_validator: with_active.then(|| ActiveCapacityValidator {
                validator_address: addr(VALIDATOR),
                epoch,
                challenged_providers: challenged.iter().map(|s| addr(s)).collect(),
                subscribed_topics: vec![],
            }),
            challenged_capacity_validators: providers,
        }
    }

    fn provider_info(address: &str) -> CapacityValidatorInfo {
        CapacityValidatorInfo {
            address: addr(address),
            stake: Coin::zero(),
            public_key: test_public_key(7),
            storage_capacity: 1_000,
            merkle_root: Some([9u8; 32]),
            seed: Some([8u8; 32]),
            chunk_count: Some(CHUNK_COUNT),
            registered_at: Some(1),
            last_merkle_root_update: Some(1),
            registered_block: 1,
            registration_duration: 1000,
        }
    }

    fn expected_challenge_id_for(provider: &str) -> ChallengeId {
        let indices = select_challenge_chunk_indices(
            EPOCH,
            &addr(provider),
            BLOCK_HEIGHT,
            &addr(VALIDATOR),
            CHUNK_COUNT,
            CHUNKS_PER_CHALLENGE,
        );
        compute_challenge_id(&addr(VALIDATOR), &addr(provider), BLOCK_HEIGHT, &indices)
    }

    fn bind_current(
        record: Option<&EpochRecord>,
        sender: &str,
        provider: &str,
        challenge_id: &str,
    ) -> Result<VerifiedProofBinding, String> {
        bind_verified_proof(
            record,
            EPOCH,
            addr(sender),
            addr(provider),
            challenge_id,
            BLOCK_HEIGHT,
            &eld_common::protocol_constants::ProtocolConstants::local_dev(),
        )
    }

    #[test]
    fn proof_epoch_window_accepts_current_and_previous_only() {
        assert_eq!(
            proof_epoch_in_window(BLOCK_HEIGHT, EPOCH, BLOCKS_PER_EPOCH).unwrap(),
            EPOCH
        );
        assert_eq!(
            proof_epoch_in_window(BLOCK_HEIGHT, EPOCH + 1, BLOCKS_PER_EPOCH).unwrap(),
            EPOCH
        );

        let older = proof_epoch_in_window(BLOCK_HEIGHT, EPOCH + 2, BLOCKS_PER_EPOCH).unwrap_err();
        assert!(older.contains("older than the previous epoch"), "{older}");

        let future = proof_epoch_in_window(BLOCK_HEIGHT, EPOCH - 1, BLOCKS_PER_EPOCH).unwrap_err();
        assert!(future.contains("after current epoch"), "{future}");
    }

    #[test]
    fn binding_accepts_matching_sender_provider_and_challenge_id() {
        let challenge_id = expected_challenge_id_for(PROVIDER);
        let record = epoch_record(EPOCH, vec![PROVIDER], vec![provider_info(PROVIDER)], true);
        let binding = bind_current(Some(&record), VALIDATOR, PROVIDER, &challenge_id.to_hex())
            .expect("binding should succeed");

        assert_eq!(binding.expected_challenge_id, challenge_id);
        assert_eq!(binding.challenged_provider_id, addr(PROVIDER));
        assert_eq!(binding.validator_address, addr(VALIDATOR));
        assert_eq!(
            binding.expected_merkle_root,
            CapacityMerkleRoot::new([9u8; 32])
        );
        assert!(!binding.expected_indices.is_empty());
    }

    #[test]
    fn binding_accepts_previous_epoch_record() {
        let challenge_id = expected_challenge_id_for(PROVIDER);
        let record = epoch_record(EPOCH, vec![PROVIDER], vec![provider_info(PROVIDER)], true);
        // Current epoch has moved on; the proof still names epoch EPOCH's validator.
        let binding = bind_verified_proof(
            Some(&record),
            EPOCH + 1,
            addr(VALIDATOR),
            addr(PROVIDER),
            &challenge_id.to_hex(),
            BLOCK_HEIGHT,
            &eld_common::protocol_constants::ProtocolConstants::local_dev(),
        )
        .expect("previous epoch proof should bind to that epoch's record");
        assert_eq!(binding.validator_address, addr(VALIDATOR));

        let err = bind_verified_proof(
            Some(&record),
            EPOCH + 1,
            addr(OTHER),
            addr(PROVIDER),
            &challenge_id.to_hex(),
            BLOCK_HEIGHT,
            &eld_common::protocol_constants::ProtocolConstants::local_dev(),
        )
        .expect_err("a different epoch's validator must not pass");
        assert!(err.contains("not active_capacity_validator"), "{err}");
    }

    #[test]
    fn binding_uses_epoch_snapshot_root_and_chunk_count() {
        const SNAPSHOT_ROOT: [u8; 32] = [4u8; 32];
        const SNAPSHOT_CHUNKS: u32 = 11;
        let provider = addr(PROVIDER);
        let validator = addr(VALIDATOR);
        let snapshot_indices = select_challenge_chunk_indices(
            EPOCH,
            &provider,
            BLOCK_HEIGHT,
            &validator,
            SNAPSHOT_CHUNKS,
            CHUNKS_PER_CHALLENGE,
        );
        let live_indices = select_challenge_chunk_indices(
            EPOCH,
            &provider,
            BLOCK_HEIGHT,
            &validator,
            CHUNK_COUNT,
            CHUNKS_PER_CHALLENGE,
        );
        assert_ne!(snapshot_indices, live_indices);
        let challenge_id =
            compute_challenge_id(&validator, &provider, BLOCK_HEIGHT, &snapshot_indices);

        let mut snapshot_provider = provider_info(PROVIDER);
        snapshot_provider.merkle_root = Some(SNAPSHOT_ROOT);
        snapshot_provider.chunk_count = Some(SNAPSHOT_CHUNKS);
        let record = epoch_record(EPOCH, vec![PROVIDER], vec![snapshot_provider], true);

        let binding = bind_current(Some(&record), VALIDATOR, PROVIDER, &challenge_id.to_hex())
            .expect("snapshot binding");

        assert_eq!(
            binding.expected_merkle_root,
            CapacityMerkleRoot::new(SNAPSHOT_ROOT)
        );
        assert_ne!(
            binding.expected_merkle_root,
            CapacityMerkleRoot::new([9u8; 32])
        );
        assert_eq!(binding.expected_indices, snapshot_indices);
    }

    #[test]
    fn binding_rejects_wrong_sender() {
        let challenge_id = expected_challenge_id_for(PROVIDER);
        let record = epoch_record(EPOCH, vec![PROVIDER], vec![provider_info(PROVIDER)], true);
        let err = bind_current(Some(&record), OTHER, PROVIDER, &challenge_id.to_hex())
            .expect_err("wrong sender");
        assert!(err.contains("not active_capacity_validator"), "{err}");
    }

    #[test]
    fn binding_rejects_provider_not_challenged() {
        let challenge_id = expected_challenge_id_for(PROVIDER);
        let record = epoch_record(EPOCH, vec![OTHER], vec![provider_info(PROVIDER)], true);
        let err = bind_current(Some(&record), VALIDATOR, PROVIDER, &challenge_id.to_hex())
            .expect_err("provider not challenged");
        assert!(err.contains("not in challenged_providers"), "{err}");
    }

    #[test]
    fn binding_rejects_fabricated_challenge_id() {
        let record = epoch_record(EPOCH, vec![PROVIDER], vec![provider_info(PROVIDER)], true);
        let err = bind_current(
            Some(&record),
            VALIDATOR,
            PROVIDER,
            "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        )
        .expect_err("fabricated challenge_id");
        assert!(err.contains("challenge_id mismatch"), "{err}");
    }

    #[test]
    fn binding_rejects_missing_epoch_record() {
        let challenge_id = expected_challenge_id_for(PROVIDER);
        let err = bind_current(None, VALIDATOR, PROVIDER, &challenge_id.to_hex())
            .expect_err("missing epoch record");
        assert!(err.contains("no epoch record"), "{err}");
    }

    #[test]
    fn binding_rejects_missing_active_capacity_validator() {
        let challenge_id = expected_challenge_id_for(PROVIDER);
        let record = epoch_record(EPOCH, vec![PROVIDER], vec![provider_info(PROVIDER)], false);
        let err = bind_current(Some(&record), VALIDATOR, PROVIDER, &challenge_id.to_hex())
            .expect_err("missing active SV");
        assert!(err.contains("no active_capacity_validator"), "{err}");
    }

    #[test]
    fn binding_challenge_id_matches_consensus_issuance_inputs() {
        let provider = addr(PROVIDER);
        let validator = addr(VALIDATOR);
        let indices = select_challenge_chunk_indices(
            EPOCH,
            &provider,
            BLOCK_HEIGHT,
            &validator,
            CHUNK_COUNT,
            CHUNKS_PER_CHALLENGE,
        );
        let issuance_challenge_id =
            compute_challenge_id(&validator, &provider, BLOCK_HEIGHT, &indices);

        let record = epoch_record(EPOCH, vec![PROVIDER], vec![provider_info(PROVIDER)], true);
        let binding = bind_current(
            Some(&record),
            VALIDATOR,
            PROVIDER,
            &issuance_challenge_id.to_hex(),
        )
        .expect("binding should accept consensus-issued challenge_id");

        assert_eq!(binding.expected_challenge_id, issuance_challenge_id);
        assert_eq!(binding.expected_indices, indices);
        assert_eq!(binding.challenged_provider_id, provider);
        assert_eq!(binding.validator_address, validator);
    }

    #[test]
    fn third_failed_proof_burns_capacity_stake_and_resets_count() {
        use crate::app_state::AppStateEnvelope;

        let provider = addr(PROVIDER);
        let mut envelope = AppStateEnvelope::default();
        let mut info = provider_info(PROVIDER);
        info.stake = Coin::new(50).expect("stake");
        envelope.capacity_validators.push(info);

        let (count, slashed) =
            record_failed_capacity_proof(&mut envelope, provider, FAILED_PROOFS_BEFORE_SLASH);
        assert_eq!((count, slashed), (1, false));
        let (count, slashed) =
            record_failed_capacity_proof(&mut envelope, provider, FAILED_PROOFS_BEFORE_SLASH);
        assert_eq!((count, slashed), (2, false));
        assert_eq!(
            envelope.capacity_validators[0].stake,
            Coin::new(50).expect("stake")
        );

        let (count, slashed) =
            record_failed_capacity_proof(&mut envelope, provider, FAILED_PROOFS_BEFORE_SLASH);
        assert_eq!((count, slashed), (0, true));
        assert!(envelope.capacity_validators[0].stake.is_zero());
        assert_eq!(envelope.failed_proof_counts.get(&provider), Some(&0));
    }
}
