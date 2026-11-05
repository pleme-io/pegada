//! Absence as evidence: present first, then absent, from a probe that answered.
//!
//! A probe that could not look ([`kotae::Outcome::Blind`]) or was not
//! understood ([`kotae::Outcome::Refused`]) proves nothing. Only an
//! [`kotae::Outcome::Empty`] answer, or a [`Sighting::Gone`] witness, proves a
//! member absent, and only after a [`Seen`] that precedes it.

use std::fmt;

use kotae::Outcome;

use crate::set::Member;

/// How strongly one kind of absence evidence binds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Admission {
    /// The evidence is the world's own statement that the member is gone.
    Proof,
    /// The evidence is admitted but can lag or lie; the reason says how.
    Mitigated(&'static str),
}

impl Admission {
    /// Whether this admission is a proof.
    #[must_use]
    pub const fn is_proof(self) -> bool {
        match self {
            Self::Proof => true,
            Self::Mitigated(_) => false,
        }
    }
}

#[doc(hidden)]
pub mod sealed {
    pub trait Sealed {}
}

/// A domain's closed set of absence evidence.
///
/// Implemented only through [`absence_witness!`], which requires every arm and
/// the empty answer to state an [`Admission`] and generates the exhaustive match.
///
/// ```compile_fail
/// use pegada::{AbsenceWitness, Admission};
/// #[derive(Debug)]
/// struct ByHand(pegada::kube::ObjectAddress);
/// impl std::fmt::Display for ByHand {
///     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.0) }
/// }
/// impl AbsenceWitness for ByHand {
///     type Member = pegada::kube::ObjectAddress;
///     fn member(&self) -> &Self::Member { &self.0 }
///     fn admission(&self) -> Admission { Admission::Proof }
///     fn empty_admission() -> Admission { Admission::Proof }
/// }
/// ```
pub trait AbsenceWitness: sealed::Sealed + fmt::Display {
    /// The member this evidence is about.
    type Member: Member;
    /// The member this evidence says is gone.
    fn member(&self) -> &Self::Member;
    /// How strongly this arm binds.
    fn admission(&self) -> Admission;
    /// How strongly an `Empty` answer from this domain's probe binds.
    fn empty_admission() -> Admission;
}

/// Declares a domain's absence witness enum and implements [`AbsenceWitness`].
///
/// Every arm carries the `member` it is about and states its admission; the
/// `empty` line states what an `Empty` answer is worth. A missing admission is
/// a macro error, and the generated matches have no wildcard.
///
/// ```
/// use pegada::{absence_witness, Admission, AbsenceWitness, kube::ObjectAddress};
/// absence_witness! {
///     /// What says a Kubernetes object is gone.
///     pub enum KubeGone for ObjectAddress {
///         empty => Admission::Proof;
///         /// A GET answered 404.
///         NotFound {} => Admission::Proof,
///         /// A label-selected LIST omitted it.
///         Unlisted { resource_version: String } => Admission::Mitigated("a list can lag"),
///     }
/// }
/// let m: ObjectAddress = "Namespace/env-1".parse().unwrap();
/// let w = KubeGone::NotFound { member: m.clone() };
/// assert_eq!(w.member(), &m);
/// assert!(w.admission().is_proof());
/// ```
///
/// An arm with no admission does not compile:
///
/// ```compile_fail
/// use pegada::{absence_witness, Admission, kube::ObjectAddress};
/// absence_witness! {
///     pub enum KubeGone for ObjectAddress {
///         empty => Admission::Proof;
///         NotFound {},
///     }
/// }
/// ```
///
/// Nor does a witness that leaves out what `Empty` is worth:
///
/// ```compile_fail
/// use pegada::{absence_witness, Admission, kube::ObjectAddress};
/// absence_witness! {
///     pub enum KubeGone for ObjectAddress {
///         NotFound {} => Admission::Proof,
///     }
/// }
/// ```
#[macro_export]
macro_rules! absence_witness {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident for $member:ty {
            empty => $empty:expr;
            $(
                $(#[$vmeta:meta])*
                $variant:ident { $($field:ident : $fty:ty),* $(,)? } => $adm:expr
            ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq)]
        $vis enum $name {
            $(
                $(#[$vmeta])*
                $variant { member: $member, $($field: $fty),* },
            )+
        }

        impl $crate::absence::sealed::Sealed for $name {}

        impl $crate::AbsenceWitness for $name {
            type Member = $member;

            fn member(&self) -> &$member {
                match self {
                    $(Self::$variant { member, .. } => member,)+
                }
            }

            fn admission(&self) -> $crate::Admission {
                match self {
                    $(Self::$variant { .. } => $adm,)+
                }
            }

            fn empty_admission() -> $crate::Admission {
                $empty
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                match self {
                    $(Self::$variant { member, .. } => {
                        write!(f, "{} {}", stringify!($variant), member)
                    })+
                }
            }
        }
    };
}

/// A member observed present at a time. Issued only by a
/// [`crate::Footprint`], from an attach or a discovery.
///
/// ```compile_fail
/// use pegada::{Seen, kube::ObjectAddress};
/// let a: ObjectAddress = "Namespace/env-1".parse().unwrap();
/// let _ = Seen { member: a, at_unix: 0 };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen<M> {
    member: M,
    at_unix: u64,
}

impl<M> Seen<M> {
    pub(crate) fn new(member: M, at_unix: u64) -> Self {
        Self { member, at_unix }
    }

    /// The member that was present.
    #[must_use]
    pub fn member(&self) -> &M {
        &self.member
    }

    /// When it was present.
    #[must_use]
    pub fn at_unix(&self) -> u64 {
        self.at_unix
    }
}

/// What a read-back probe saw about one member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sighting<W> {
    /// The member is gone, and this is the evidence.
    Gone(W),
    /// The member is still there.
    Standing,
}

/// What an [`Absent`] stands on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evidence<W> {
    /// A domain witness.
    Witness(W),
    /// A probe that answered and found nothing.
    Empty {
        /// What was looked for.
        of: String,
    },
}

/// A member proven absent after it was seen present. Constructible only by
/// [`prove_absent`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_field_names)]
pub struct Absent<W: AbsenceWitness> {
    member: W::Member,
    evidence: Evidence<W>,
    admission: Admission,
    present_seen_at: u64,
    absent_seen_at: u64,
}

impl<W: AbsenceWitness> Absent<W> {
    /// The member proven absent.
    #[must_use]
    pub fn member(&self) -> &W::Member {
        &self.member
    }

    /// What the proof stands on.
    #[must_use]
    pub fn evidence(&self) -> &Evidence<W> {
        &self.evidence
    }

    /// How strongly it binds.
    #[must_use]
    pub fn admission(&self) -> Admission {
        self.admission
    }

    /// When the member was last seen present.
    #[must_use]
    pub fn present_seen_at(&self) -> u64 {
        self.present_seen_at
    }

    /// When the member was seen absent.
    #[must_use]
    pub fn absent_seen_at(&self) -> u64 {
        self.absent_seen_at
    }
}

/// Why a probe did not prove a member absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unproven {
    /// No presence precedes this absence: the witness names a different
    /// member, or the absence was observed before the presence.
    NeverSeenPresent,
    /// The probe could not look. Nothing is concluded about the member.
    Blind(String),
    /// The probe was not understood. Nothing is concluded about the member.
    Refused(String),
    /// The probe answered and the member is still there.
    StillPresent,
}

impl fmt::Display for Unproven {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NeverSeenPresent => f.write_str("no presence precedes this absence"),
            Self::Blind(why) => write!(f, "blind: {why}"),
            Self::Refused(why) => write!(f, "refused: {why}"),
            Self::StillPresent => f.write_str("still present"),
        }
    }
}

impl std::error::Error for Unproven {}

/// Proves one member absent: it was `present`, and a probe that answered at
/// `absent_seen_at` says it is gone.
///
/// The match over the probe's answer is exhaustive. `Blind` and `Refused`
/// never count.
///
/// # Errors
///
/// [`Unproven`] says why the answer is not a proof: the probe was blind or
/// refused, the member still stands, or no presence precedes the absence.
pub fn prove_absent<W: AbsenceWitness>(
    present: Seen<W::Member>,
    probe: Outcome<Sighting<W>>,
    absent_seen_at: u64,
) -> Result<Absent<W>, Unproven> {
    let (evidence, admission) = match probe {
        Outcome::Found(Sighting::Gone(witness)) => {
            if witness.member() != &present.member {
                return Err(Unproven::NeverSeenPresent);
            }
            let admission = witness.admission();
            (Evidence::Witness(witness), admission)
        }
        Outcome::Found(Sighting::Standing) => return Err(Unproven::StillPresent),
        Outcome::Empty { of } => (Evidence::Empty { of }, W::empty_admission()),
        Outcome::Refused { because, .. } => return Err(Unproven::Refused(because)),
        Outcome::Blind { because } => return Err(Unproven::Blind(because)),
    };
    if absent_seen_at < present.at_unix {
        return Err(Unproven::NeverSeenPresent);
    }
    Ok(Absent {
        member: present.member,
        evidence,
        admission,
        present_seen_at: present.at_unix,
        absent_seen_at,
    })
}
