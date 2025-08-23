#![cfg_attr(not(any(test, feature = "export-abi")), no_main)]
extern crate alloc;

use alloy_primitives::{Address, B256, U256};
use stylus_sdk::{abi::Bytes, prelude::*, stylus_core::calls::context::Call};

sol_storage! {
    #[entrypoint]
    pub struct StylusTestProgramWithPayableConstructor {
        uint256 number;
    }
}

#[public]
impl StylusTestProgramWithPayableConstructor {
    #[constructor]
    #[payable]
    pub fn constructor(&mut self, value: U256) {
        self.number.set(value);
    }

    pub fn number(&self) -> U256 {
        self.number.get()
    }

    #[payable]
    fn call(
        &mut self,
        target: Address,
        data: Bytes,
        value: U256,
        gas_limit: U256,
    ) -> Result<(Bytes, U256), Vec<u8>> {
        let mut raw_call = Call::new().value(value);

        if !gas_limit.is_zero() {
            raw_call = raw_call.gas(gas_limit.as_limbs()[0]);
        }

        let ink_left = self.vm().evm_ink_left();
        let result = self.vm().call(&raw_call, target, data.as_ref())?;
        let ink_used = ink_left - self.vm().evm_ink_left();

        Ok((Bytes::from(result), U256::from(ink_used)))
    }

    fn delegate_call(
        &mut self,
        target: Address,
        data: Bytes,
        gas_limit: U256,
    ) -> Result<(Bytes, U256), Vec<u8>> {
        let mut raw_call = Call::new();

        if !gas_limit.is_zero() {
            raw_call = raw_call.gas(gas_limit.as_limbs()[0]);
        }

        let ink_left = self.vm().evm_ink_left();
        let result = unsafe { self.vm().delegate_call(&raw_call, target, data.as_ref())? };
        let ink_used = ink_left - self.vm().evm_ink_left();

        Ok((Bytes::from(result), U256::from(ink_used)))
    }

    fn static_call(
        &self,
        target: Address,
        data: Bytes,
        gas_limit: U256,
    ) -> Result<(Bytes, U256), Vec<u8>> {
        let mut raw_call = Call::new();

        if !gas_limit.is_zero() {
            raw_call = raw_call.gas(gas_limit.as_limbs()[0]);
        }

        let ink_left = self.vm().evm_ink_left();
        let result = self.vm().static_call(&raw_call, target, data.as_ref())?;
        let ink_used = ink_left - self.vm().evm_ink_left();

        Ok((Bytes::from(result), U256::from(ink_used)))
    }

    fn sstore(&mut self, key: U256, value: U256) -> U256 {
        let ink_left = self.vm().evm_ink_left();
        unsafe {
            self.vm().storage_cache_bytes32(key, value.into());
        }

        let ink_used = ink_left - self.vm().evm_ink_left();
        U256::from(ink_used)
    }

    fn sload(&self, key: U256) -> (U256, U256) {
        let ink_left = self.vm().evm_ink_left();
        let result = self.vm().storage_load_bytes32(key);
        let ink_used = ink_left - self.vm().evm_ink_left();
        (result.into(), U256::from(ink_used))
    }

    fn log(&self, topics: Vec<B256>, data: Bytes) -> U256 {
        let ink_left = self.vm().evm_ink_left();
        self.vm().raw_log(topics.as_slice(), data.as_ref()).unwrap();
        let ink_used = ink_left - self.vm().evm_ink_left();
        U256::from(ink_used)
    }

    #[payable]
    fn create(&self, code: Bytes, value: U256) -> Result<(Address, U256), Vec<u8>> {
        let ink_left = self.vm().evm_ink_left();
        let result = unsafe { self.vm().deploy(code.as_ref(), value, None)? };
        let ink_used = ink_left - self.vm().evm_ink_left();
        Ok((result, U256::from(ink_used)))
    }

    fn account_balance(&self, address: Address) -> (U256, U256) {
        let ink_left = self.vm().evm_ink_left();
        let balance = self.vm().balance(address);
        let ink_used = ink_left - self.vm().evm_ink_left();
        (balance, U256::from(ink_used))
    }

    fn account_code(&self, address: Address) -> (Bytes, U256) {
        let ink_left = self.vm().evm_ink_left();
        let code = self.vm().code(address).to_vec();
        let ink_used = ink_left - self.vm().evm_ink_left();

        (Bytes::from(code), U256::from(ink_used))
    }

    fn account_code_hash(&self, address: Address) -> (B256, U256) {
        let ink_left = self.vm().evm_ink_left();
        let code_hash = self.vm().code_hash(address);
        let ink_used = ink_left - self.vm().evm_ink_left();
        (code_hash, U256::from(ink_used))
    }

    fn ping(&self) -> Bytes {
        Bytes::from("this is a really long response that should be returned by the ping function to test the multicall functionality".as_bytes().to_vec())
    }
}
