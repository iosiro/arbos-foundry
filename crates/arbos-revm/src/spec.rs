use core::str::FromStr;
use revm::primitives::hardfork::{SpecId, UnknownHardfork};

#[repr(u8)]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(non_camel_case_types)]
pub enum ArbitrumSpecId {
    ArbosFixRedeemGas = 11,
    ArbosStylus = 30,
    ArbosStylusFixes = 31,
    #[default]
    ArbosStylusChargingFixes = 32,
}

impl ArbitrumSpecId {
    /// Converts the [`ArbitrumSpecId`] into a [`SpecId`].
    pub const fn into_eth_spec(self) -> SpecId {
        match self {
            Self::ArbosFixRedeemGas
            | Self::ArbosStylus
            | Self::ArbosStylusFixes
            | Self::ArbosStylusChargingFixes => SpecId::PRAGUE,
        }
    }

    pub const fn is_enabled_in(self, other: Self) -> bool {
        other as u8 <= self as u8
    }
}

impl From<ArbitrumSpecId> for SpecId {
    fn from(spec: ArbitrumSpecId) -> Self {
        spec.into_eth_spec()
    }
}

impl FromStr for ArbitrumSpecId {
    type Err = UnknownHardfork;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            name::ARBOS_FIX_REDEEM_GAS => Ok(Self::ArbosFixRedeemGas),
            name::ARBOS_STYLUS => Ok(Self::ArbosStylus),
            name::ARBOS_STYLUS_FIXES => Ok(Self::ArbosStylusFixes),
            name::ARBOS_STYLUS_CHARGING_FIXES => Ok(Self::ArbosStylusChargingFixes),
            _ => Err(UnknownHardfork),
        }
    }
}

impl From<ArbitrumSpecId> for &'static str {
    fn from(spec_id: ArbitrumSpecId) -> Self {
        match spec_id {
            ArbitrumSpecId::ArbosFixRedeemGas => name::ARBOS_FIX_REDEEM_GAS,
            ArbitrumSpecId::ArbosStylus => name::ARBOS_STYLUS,
            ArbitrumSpecId::ArbosStylusFixes => name::ARBOS_STYLUS_FIXES,
            ArbitrumSpecId::ArbosStylusChargingFixes => name::ARBOS_STYLUS_CHARGING_FIXES,
        }
    }
}

/// String identifiers for Arbitrum hardforks
pub mod name {
    pub const ARBOS_FIX_REDEEM_GAS: &str = "arbos11";
    pub const ARBOS_STYLUS: &str = "arbos30";
    pub const ARBOS_STYLUS_FIXES: &str = "arbos31";
    pub const ARBOS_STYLUS_CHARGING_FIXES: &str = "arbos32";
}
