use std::fmt::Display;

use revm::{
    context::JournalTr,
    interpreter::{
        Gas, InstructionResult, InterpreterAction, InterpreterResult,
        gas::{ISTANBUL_SLOAD_GAS, SSTORE_RESET, SSTORE_SET},
    },
    primitives::{
        Address, B256, Bytes, FixedBytes, I256, U256,
        alloy_primitives::Signed,
        keccak256,
        ruint::{Uint, UintTryTo},
    },
};

use crate::{ArbitrumContextTr, constants::ARBOS_STATE_ADDRESS};
use tracing::trace;
#[derive(Debug)]
pub enum ArbosStateError {
    OutOfGas,
    StateChangeDuringStaticCall,
    InvalidBlockNumberForBlockHash,
    DecompressError(String),
    ProgramNotActivated,
    ProgramNeedsUpgrade(u16, u16),
    ProgramExpired(u32),
    Context(String),
}

impl Display for ArbosStateError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::OutOfGas => write!(f, "Out of gas"),
            Self::StateChangeDuringStaticCall => {
                write!(f, "State change attempted during static call")
            }
            Self::InvalidBlockNumberForBlockHash => {
                write!(f, "Invalid block number for block hash")
            }
            Self::DecompressError(msg) => {
                write!(f, "Decompression error: {msg}")
            }
            Self::ProgramNotActivated => {
                write!(f, "Program not activated")
            }
            Self::ProgramNeedsUpgrade(current, required) => {
                write!(f, "Program needs upgrade from version {current} to {required}")
            }
            Self::ProgramExpired(expired_at) => {
                write!(f, "Program expired at {expired_at}")
            }
            Self::Context(err) => {
                write!(f, "Context error: {err}")
            }
        }
    }
}

impl From<ArbosStateError> for String {
    fn from(error: ArbosStateError) -> Self {
        error.to_string()
    }
}

impl From<ArbosStateError> for Bytes {
    fn from(error: ArbosStateError) -> Self {
        Self::from(error.to_string().into_bytes())
    }
}

impl From<ArbosStateError> for InterpreterAction {
    fn from(error: ArbosStateError) -> Self {
        Self::Return(error.into())
    }
}

impl From<ArbosStateError> for InstructionResult {
    fn from(error: ArbosStateError) -> Self {
        let description = error.to_string();
        let result = match error {
            ArbosStateError::OutOfGas => Self::OutOfGas,
            ArbosStateError::StateChangeDuringStaticCall => Self::StateChangeDuringStaticCall,
            ArbosStateError::Context(_) => Self::Revert,
            _ => Self::Revert,
        };

        trace!(
            target: "arbos-revm::arbos_state",
            %description,
            ?result,
            "Converted ArbosStateError into InstructionResult"
        );

        result
    }
}

impl From<ArbosStateError> for InterpreterResult {
    fn from(error: ArbosStateError) -> Self {
        let description = error.to_string();
        let result = match error {
            ArbosStateError::OutOfGas => Self {
                result: InstructionResult::OutOfGas,
                gas: Gas::default(),
                output: Bytes::default(),
            },
            ArbosStateError::StateChangeDuringStaticCall => Self {
                result: InstructionResult::StateChangeDuringStaticCall,
                gas: Gas::default(),
                output: Bytes::default(),
            },
            _ => Self {
                result: InstructionResult::Revert,
                gas: Gas::default(),
                output: Bytes::default(),
            },
        };

        trace!(
            target: "arbos-revm::arbos_state",
            %description,
            result = ?result.result,
            output_len = result.output.len(),
            "Converted ArbosStateError into InterpreterResult"
        );

        result
    }
}

pub struct StorageBacked<'a, CTX, T> {
    pub context: &'a mut CTX,
    pub gas: Option<&'a mut Gas>,
    pub is_static: bool,
    pub slot: B256,
    _marker: core::marker::PhantomData<T>,
}

pub trait StorageBackedTr<'a, CTX, T>
where
    CTX: ArbitrumContextTr,
    T: StorageWord,
{
    fn new(context: &'a mut CTX, gas: Option<&'a mut Gas>, is_static: bool, slot: B256) -> Self;
    fn get(&mut self) -> Result<T, ArbosStateError>;
    fn set(&mut self, value: T) -> Result<(), ArbosStateError>;
}

// --- utility helpers moved to module scope ---
pub fn substorage(root: &B256, index: &[u8]) -> B256 {
    let mut subkey_bytes =
        if root.is_zero() { Vec::with_capacity(1) } else { root.as_slice().to_vec() };
    subkey_bytes.extend_from_slice(index);
    keccak256(subkey_bytes)
}

pub fn map_address(storage_key: &B256, key: &B256) -> B256 {
    let key_bytes = key.as_slice();
    let boundary = key_bytes.len() - 1;

    let mut to_hash = Vec::with_capacity(storage_key.len() + boundary);
    if !storage_key.is_zero() {
        to_hash.extend_from_slice(storage_key.as_slice());
    }
    to_hash.extend_from_slice(&key_bytes[..boundary]);

    let digest = keccak256(&to_hash);

    let mut mapped = digest[..boundary].to_vec();
    mapped.push(key_bytes[boundary]);
    B256::from_slice(&mapped)
}

pub type StorageBackedU256<'a, CTX> = StorageBacked<'a, CTX, U256>;
pub type StorageBackedU32<'a, CTX> = StorageBacked<'a, CTX, u32>;
pub type StorageBackedU64<'a, CTX> = StorageBacked<'a, CTX, u64>;
pub type StorageBackedI256<'a, CTX> = StorageBacked<'a, CTX, I256>;
pub type StorageBackedAddress<'a, CTX> = StorageBacked<'a, CTX, Address>;
pub type StorageBackedB256<'a, CTX> = StorageBacked<'a, CTX, B256>;

/// Anything that can round-trip through a single storage word (U256).
pub trait StorageWord: Sized {
    /// Decode from the raw storage word.
    fn from_word(word: U256) -> Self;

    /// Encode into the raw storage word.
    fn into_word(self) -> U256;
}

impl<const BITS: usize, const LIMBS: usize> StorageWord for Uint<BITS, LIMBS>
where
    // U256 → this Uint
    U256: UintTryTo<Self>,
    // this Uint → U256
    Self: UintTryTo<U256>,
{
    fn from_word(word: U256) -> Self {
        // Saturating, so small types don't panic on overflow
        word.to::<Self>()
    }

    fn into_word(self) -> U256 {
        self.to::<U256>()
    }
}

impl<const BITS: usize, const LIMBS: usize> StorageWord for Signed<BITS, LIMBS>
where
    // U256 → this Uint
    U256: UintTryTo<Uint<BITS, LIMBS>>,
    // this Uint → U256
    Uint<BITS, LIMBS>: UintTryTo<U256>,
{
    fn from_word(word: U256) -> Self {
        // Saturating, so small types don't panic on overflow
        let word = word.to::<Uint<BITS, LIMBS>>();
        Self::from_raw(word)
    }

    fn into_word(self) -> U256 {
        U256::from(self.into_raw())
    }
}

impl<const N: usize> StorageWord for FixedBytes<N>
where
    // these bounds express “Alloy knows how to map
    // between this and B256”
    B256: From<Self>,
    Self: From<B256>,
{
    fn from_word(word: U256) -> Self {
        Self(word.to_be_bytes())
    }

    fn into_word(self) -> U256 {
        let b256: B256 = B256::from(self);
        U256::from_be_slice(b256.as_slice())
    }
}

impl StorageWord for Address {
    fn from_word(word: U256) -> Self {
        let b256: B256 = B256::from(FixedBytes(word.to_be_bytes()));
        Self::from_slice(&b256.as_slice()[12..32])
    }

    fn into_word(self) -> U256 {
        let b256 = B256::from(U256::from_be_slice(self.as_slice()));
        U256::from_be_slice(b256.as_slice())
    }
}

impl StorageWord for u32 {
    fn from_word(word: U256) -> Self {
        word.saturating_to()
    }

    fn into_word(self) -> U256 {
        U256::from(self)
    }
}

impl StorageWord for u64 {
    fn from_word(word: U256) -> Self {
        word.saturating_to()
    }

    fn into_word(self) -> U256 {
        U256::from(self)
    }
}

impl<'a, CTX, T> StorageBackedTr<'a, CTX, T> for StorageBacked<'a, CTX, T>
where
    CTX: ArbitrumContextTr,
    T: StorageWord,
{
    fn new(context: &'a mut CTX, gas: Option<&'a mut Gas>, is_static: bool, slot: B256) -> Self {
        Self { context, gas, is_static, slot, _marker: std::marker::PhantomData }
    }

    fn get(&mut self) -> Result<T, ArbosStateError> {
        if let Some(gas) = &mut self.gas
            && !gas.record_cost(ISTANBUL_SLOAD_GAS)
        {
            return Err(ArbosStateError::OutOfGas);
        }

        // Assuming your journal sload returns something with `.data: U256`
        let word: U256 = self
            .context
            .journal_mut()
            .sload(ARBOS_STATE_ADDRESS, self.slot.into())
            .map(|s| s.data)
            .map_err(|err| ArbosStateError::Context(err.to_string()))?;

        Ok(T::from_word(word))
    }

    fn set(&mut self, value: T) -> Result<(), ArbosStateError> {
        if self.is_static {
            return Err(ArbosStateError::StateChangeDuringStaticCall);
        }

        let value = value.into_word();

        if let Some(gas) = &mut self.gas {
            if value.is_zero() {
                if !gas.record_cost(SSTORE_RESET) {
                    return Err(ArbosStateError::OutOfGas);
                }
            } else if !gas.record_cost(SSTORE_SET) {
                return Err(ArbosStateError::OutOfGas);
            }
        }

        _ = self.context.sstore(ARBOS_STATE_ADDRESS, self.slot.into(), value);

        Ok(())
    }
}
/// Storage-backed address set implemented as array-with-length at index 0. Values are left-padded
/// B256.
pub struct StorageBackedAddressSet<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub context: &'a mut CTX,
    pub gas: Option<&'a mut Gas>,
    pub is_static: bool,
    pub slot: B256,
}

impl<'a, CTX> StorageBackedAddressSet<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub fn new(
        context: &'a mut CTX,
        gas: Option<&'a mut Gas>,
        is_static: bool,
        slot: B256,
    ) -> Self {
        Self { context, gas, is_static, slot }
    }

    fn size_slot(&self) -> B256 {
        map_address(&self.slot, &B256::from(U256::from(0u64)))
    }

    pub fn size(&mut self) -> Result<usize, ArbosStateError> {
        let size_slot = self.size_slot();
        StorageBackedU256::new(self.context, self.gas.as_deref_mut(), self.is_static, size_slot)
            .get()
            .map(|v| v.saturating_to::<usize>())
    }

    pub fn all(&mut self) -> Result<Vec<Address>, ArbosStateError> {
        let n = self.size()?;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let slot = map_address(&self.slot, &B256::from(U256::from(i as u64 + 1)));
            let addr = StorageBackedAddress::new(
                self.context,
                self.gas.as_deref_mut(),
                self.is_static,
                slot,
            )
            .get()?;
            out.push(addr);
        }
        Ok(out)
    }

    pub fn contains(&mut self, address: Address) -> Result<bool, ArbosStateError> {
        let by_address = substorage(&self.slot, &[0]);
        let slot = map_address(&by_address, &B256::left_padding_from(address.as_slice()));
        let addr =
            StorageBackedAddress::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
                .get()?;
        Ok(!addr.is_zero())
    }

    pub fn add(&mut self, address: Address) -> Result<(), ArbosStateError> {
        // push to array
        let size = {
            let size_slot = self.size_slot();
            let mut size_slot = StorageBackedU256::new(
                self.context,
                self.gas.as_deref_mut(),
                self.is_static,
                size_slot,
            );
            let size = size_slot.get()? + U256::ONE;
            size_slot.set(size)?;
            size
        };

        let slot = map_address(&self.slot, &B256::from(size));
        StorageBackedAddress::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
            .set(address)?;

        // also set by-address index so contains() is O(1)
        let by_address = substorage(&self.slot, &[0]);
        StorageBackedU256::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            map_address(&by_address, &B256::left_padding_from(address.as_slice())),
        )
        .set(U256::ONE)?;

        Ok(())
    }

    pub fn remove(&mut self, address: &Address) -> Result<(), ArbosStateError> {
        let by_address = substorage(&self.slot, &[0]);
        let by_address_slot =
            map_address(&by_address, &B256::left_padding_from(address.as_slice()));

        StorageBackedU256::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            by_address_slot,
        )
        .set(U256::ZERO)

        // NOTE: we don't compact the array in storage to keep logic simple and predictable.
    }
}

pub struct StorageBackedBytes<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub context: &'a mut CTX,
    pub gas: Option<&'a mut Gas>,
    pub is_static: bool,
    pub slot: B256,
}

impl<'a, CTX> StorageBackedBytes<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub fn new(
        context: &'a mut CTX,
        gas: Option<&'a mut Gas>,
        is_static: bool,
        slot: B256,
    ) -> Self {
        Self { context, gas, is_static, slot }
    }

    pub fn get(&mut self) -> Result<Vec<u8>, ArbosStateError> {
        let size_slot = map_address(&self.slot, &B256::from(U256::from(0u64)));
        let size = StorageBackedU256::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            size_slot,
        )
        .get()?;

        let size = size.to::<usize>();

        let mut out = Vec::with_capacity(size as usize);
        let mut offset = 0;
        while offset < size {
            let chunk_slot = map_address(&self.slot, &B256::from(U256::from(offset + 1)));
            let chunk = StorageBackedB256::new(
                self.context,
                self.gas.as_deref_mut(),
                self.is_static,
                chunk_slot,
            )
            .get()?;

            let chunk_bytes = chunk.to_vec();
            let to_copy = std::cmp::min(size - offset, 32);
            out.extend_from_slice(&chunk_bytes[..to_copy as usize]);
            offset += to_copy;
        }
        Ok(out)
    }

    pub fn set(&mut self, value: &[u8]) -> Result<(), ArbosStateError> {
        let size_slot = map_address(&self.slot, &B256::from(U256::from(0u64)));
        StorageBackedU256::new(self.context, self.gas.as_deref_mut(), self.is_static, size_slot)
            .set(U256::from(value.len() as u64))?;

        let mut offset = 0;
        while offset < value.len() {
            let chunk_slot = map_address(&self.slot, &B256::from(U256::from(offset + 1)));

            let to_copy = std::cmp::min(value.len() - offset, 32);
            let mut chunk_bytes = [0u8; 32];
            chunk_bytes[..to_copy].copy_from_slice(&value[offset..(offset + to_copy)]);
            let chunk = B256::from_slice(&chunk_bytes);
            StorageBackedB256::new(
                self.context,
                self.gas.as_deref_mut(),
                self.is_static,
                chunk_slot,
            )
            .set(chunk)?;
            offset += to_copy;
        }
        Ok(())
    }
}

pub struct StorageBackedQueue<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub context: &'a mut CTX,
    pub gas: Option<&'a mut Gas>,
    pub is_static: bool,
    pub slot: B256,
}

impl<'a, CTX> StorageBackedQueue<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub fn new(
        context: &'a mut CTX,
        gas: Option<&'a mut Gas>,
        is_static: bool,
        slot: B256,
    ) -> Self {
        Self { context, gas, is_static, slot }
    }

    fn head_slot(&self) -> B256 {
        map_address(&self.slot, &B256::from(U256::from(0u64)))
    }

    fn tail_slot(&self) -> B256 {
        map_address(&self.slot, &B256::from(U256::from(1u64)))
    }

    pub fn size(&mut self) -> Result<u64, ArbosStateError> {
        let head_slot = self.head_slot();
        let tail_slot = self.tail_slot();

        let head =
            StorageBackedU64::new(self.context, self.gas.as_deref_mut(), self.is_static, head_slot)
                .get()?;
        let tail =
            StorageBackedU64::new(self.context, self.gas.as_deref_mut(), self.is_static, tail_slot)
                .get()?;
        Ok(tail.saturating_sub(head))
    }

    pub fn peek(&mut self) -> Result<Option<U256>, ArbosStateError> {
        let head_slot = self.head_slot();
        let tail_slot = self.tail_slot();

        let head =
            StorageBackedU64::new(self.context, self.gas.as_deref_mut(), self.is_static, head_slot)
                .get()?;
        let tail =
            StorageBackedU64::new(self.context, self.gas.as_deref_mut(), self.is_static, tail_slot)
                .get()?;

        if head >= tail {
            return Ok(None);
        }
        let elem_slot = map_address(&self.slot, &B256::from(U256::from(head)));

        let v = StorageBackedU256::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            elem_slot,
        )
        .get()?;
        Ok(Some(v))
    }

    pub fn pop(&mut self) -> Result<Option<U256>, ArbosStateError> {
        let head_slot = self.head_slot();
        let tail_slot = self.tail_slot();

        let head =
            StorageBackedU64::new(self.context, self.gas.as_deref_mut(), self.is_static, head_slot)
                .get()?;
        let tail =
            StorageBackedU64::new(self.context, self.gas.as_deref_mut(), self.is_static, tail_slot)
                .get()?;

        if head >= tail {
            return Ok(None);
        }
        let elem_slot = map_address(&self.slot, &B256::from(U256::from(head)));
        let v = StorageBackedU256::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            elem_slot,
        )
        .get()?;

        // increment head
        let new_head = head.saturating_add(1);
        StorageBackedU64::new(self.context, self.gas.as_deref_mut(), self.is_static, head_slot)
            .set(new_head)?;
        Ok(Some(v))
    }

    pub fn push(&mut self, value: U256) -> Result<(), ArbosStateError> {
        let tail_slot = self.tail_slot();

        let tail =
            StorageBackedU64::new(self.context, self.gas.as_deref_mut(), self.is_static, tail_slot)
                .get()?;
        let new_tail = tail.saturating_add(1);
        StorageBackedU64::new(self.context, self.gas.as_deref_mut(), self.is_static, tail_slot)
            .set(new_tail)?;

        let elem_slot = map_address(&self.slot, &B256::from(U256::from(tail)));
        StorageBackedU256::new(self.context, self.gas.as_deref_mut(), self.is_static, elem_slot)
            .set(value)?;

        Ok(())
    }
}
