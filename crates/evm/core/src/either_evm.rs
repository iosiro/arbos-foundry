use alloy_evm::{Database, EthEvm};
use core::ops::{Deref, DerefMut};

pub struct EitherEvm<DB, I, P>(pub EthEvm<DB, I, P>)
where
    DB: Database;

impl<DB, I, P> Deref for EitherEvm<DB, I, P>
where
    DB: Database,
{
    type Target = EthEvm<DB, I, P>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<DB, I, P> DerefMut for EitherEvm<DB, I, P>
where
    DB: Database,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
