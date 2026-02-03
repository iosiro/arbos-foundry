//! A simple wrapper around EVM implementations.
//!
//! This module provides a thin wrapper that can be used to unify different EVM implementations
//! or add additional functionality around them.

use core::ops::{Deref, DerefMut};

/// A wrapper around an EVM implementation.
///
/// This is used to provide a consistent interface for different EVM implementations.
pub struct EitherEvm<Evm>(pub Evm);

impl<Evm> Deref for EitherEvm<Evm> {
    type Target = Evm;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Evm> DerefMut for EitherEvm<Evm> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
