//! The good terminal: every owned member proven gone.

use std::fmt;

use tameshi::content_address::ContentAddress;

use crate::set::NonEmpty;

/// The weakest admission among the proofs a [`Reaped`] stands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tier {
    /// Every member's absence is a proof.
    Proven,
    /// At least one member's absence is admitted but mitigated.
    OnlyMitigated,
}

impl Tier {
    /// Every tier, in declaration order.
    pub const ALL: &'static [Self] = &[Self::Proven, Self::OnlyMitigated];

    /// The stable text form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Proven => "proven",
            Self::OnlyMitigated => "only-mitigated",
        }
    }
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A footprint whose every member is proven absent. Its sole constructor is
/// [`crate::Footprint::into_reaped`].
///
/// ```compile_fail
/// use pegada::{Reaped, NonEmpty, Tier, kube::ObjectAddress};
/// let a: ObjectAddress = "Namespace/env-1".parse().unwrap();
/// let digest = tameshi::content_address::ContentAddress::of_json(&1u8).unwrap();
/// let _ = Reaped { members: NonEmpty::new(a, []), digest, tier: Tier::Proven };
/// ```
///
/// ```compile_fail
/// use pegada::{Reaped, NonEmpty, Tier, kube::ObjectAddress};
/// let a: ObjectAddress = "Namespace/env-1".parse().unwrap();
/// let digest = tameshi::content_address::ContentAddress::of_json(&1u8).unwrap();
/// let _ = Reaped::new(NonEmpty::new(a, []), digest, Tier::Proven);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reaped<M> {
    members: NonEmpty<M>,
    digest: ContentAddress,
    tier: Tier,
}

impl<M> Reaped<M> {
    pub(crate) fn new(members: NonEmpty<M>, digest: ContentAddress, tier: Tier) -> Self {
        Self {
            members,
            digest,
            tier,
        }
    }

    /// The members proven gone.
    #[must_use]
    pub fn members(&self) -> &NonEmpty<M> {
        &self.members
    }

    /// The content address of the members and the proofs.
    #[must_use]
    pub fn digest(&self) -> ContentAddress {
        self.digest
    }

    /// How strongly the proofs bind.
    #[must_use]
    pub fn tier(&self) -> Tier {
        self.tier
    }
}

/// The members a reap attempt could not prove gone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outstanding<M> {
    /// Owned and not proven absent.
    pub not_proven: NonEmpty<M>,
}

impl<M: fmt::Display> fmt::Display for Outstanding<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} member(s) not proven absent:", self.not_proven.len())?;
        for m in &self.not_proven {
            write!(f, " {m}")?;
        }
        Ok(())
    }
}

impl<M: fmt::Debug + fmt::Display> std::error::Error for Outstanding<M> {}
