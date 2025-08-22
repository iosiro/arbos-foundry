use revm::{context_interface::result::HaltReason, interpreter::InstructionResult};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ArbitrumHaltReason(pub HaltReason);

impl From<HaltReason> for ArbitrumHaltReason {
    fn from(value: HaltReason) -> Self {
        Self(value)
    }
}

impl From<ArbitrumHaltReason> for HaltReason {
    fn from(val: ArbitrumHaltReason) -> Self {
        val.0
    }
}

impl From<ArbitrumHaltReason> for InstructionResult {
    fn from(val: ArbitrumHaltReason) -> Self {
        val.0.into()
    }
}
