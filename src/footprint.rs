//! The owned set: recorded at create time, cross-checked by discovery.
//!
//! A [`Footprint`] has no constructor from a hand-written list. Members enter
//! by [`Footprint::attach`] (the creating call's response) or by
//! [`Footprint::reconcile`] (a differently shaped discovery probe). It is
//! never empty: it opens on its first member.

use std::collections::BTreeMap;
use std::fmt;

use kotae::Outcome;
use serde::Serialize;
use tameshi::content_address::ContentAddress;

use crate::absence::{AbsenceWitness, Absent, Admission, Seen};
use crate::reaped::{Outstanding, Reaped, Tier};
use crate::set::{Member, NonEmpty, missing_from};

/// The receipt of one attach: this member entered the footprint at this time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attached<M> {
    member: M,
    at_unix: u64,
}

impl<M: Clone> Attached<M> {
    /// The attached member.
    #[must_use]
    pub fn member(&self) -> &M {
        &self.member
    }

    /// When it was attached.
    #[must_use]
    pub fn at_unix(&self) -> u64 {
        self.at_unix
    }

    /// The presence this attach witnessed.
    #[must_use]
    pub fn seen(&self) -> Seen<M> {
        Seen::new(self.member.clone(), self.at_unix)
    }
}

/// The name of a discovery probe, in its own words (a label selector, a tag
/// filter).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Discovery(String);

impl Discovery {
    /// Names a discovery probe.
    #[must_use]
    pub fn new(by: impl Into<String>) -> Self {
        Self(by.into())
    }

    /// The probe's name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Discovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What one discovery probe found. An empty find is `Outcome::Empty`, never
/// a `Discovered`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovered<M> {
    members: NonEmpty<M>,
    by: Discovery,
    at_unix: u64,
}

impl<M> Discovered<M> {
    /// The members `by` saw present at `at_unix`.
    #[must_use]
    pub fn new(members: NonEmpty<M>, by: Discovery, at_unix: u64) -> Self {
        Self {
            members,
            by,
            at_unix,
        }
    }

    /// The members found.
    #[must_use]
    pub fn members(&self) -> &NonEmpty<M> {
        &self.members
    }

    /// The probe that found them.
    #[must_use]
    pub fn by(&self) -> &Discovery {
        &self.by
    }

    /// When.
    #[must_use]
    pub fn at_unix(&self) -> u64 {
        self.at_unix
    }
}

/// The two set differences of one reconcile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drift<M> {
    /// Discovered but never attached: created without a record.
    pub orphans: Vec<M>,
    /// Attached but not discovered: recorded, and the probe cannot see it.
    pub unobserved: Vec<M>,
    /// The probe did not answer; no difference was taken and nothing changed.
    pub blind: Option<String>,
}

/// What a lifecycle owns in the world.
///
/// ```compile_fail
/// use pegada::{Footprint, kube::ObjectAddress};
/// let a: ObjectAddress = "Namespace/env-1".parse().unwrap();
/// let _: Footprint<ObjectAddress> = Footprint::from(vec![a]);
/// ```
///
/// ```compile_fail
/// use pegada::{Footprint, kube::ObjectAddress};
/// let _: Footprint<ObjectAddress> = Footprint { attached: Default::default(), discovered: Default::default() };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Footprint<M: Member> {
    attached: BTreeMap<M, u64>,
    discovered: BTreeMap<M, u64>,
}

impl<M: Member> Footprint<M> {
    /// Opens a footprint on its first created member.
    #[must_use]
    pub fn open(first: M, at_unix: u64) -> (Self, Attached<M>) {
        let mut footprint = Self {
            attached: BTreeMap::new(),
            discovered: BTreeMap::new(),
        };
        let attached = footprint.attach(first, at_unix);
        (footprint, attached)
    }

    /// Opens a footprint on what a discovery probe found: nothing attached,
    /// every member owned as discovered.
    #[must_use]
    pub fn discover(found: Discovered<M>) -> Self {
        let at = found.at_unix;
        Self {
            attached: BTreeMap::new(),
            discovered: found
                .members
                .into_vec()
                .into_iter()
                .map(|m| (m, at))
                .collect(),
        }
    }

    /// Records a member at the moment its creating call returned.
    pub fn attach(&mut self, member: M, at_unix: u64) -> Attached<M> {
        self.attached.entry(member.clone()).or_insert(at_unix);
        Attached { member, at_unix }
    }

    /// Cross-checks the attached set against a discovery probe's answer.
    ///
    /// `Found` and `Empty` take both differences; discovered members join the
    /// owned set. `Blind` and `Refused` change nothing and say so.
    pub fn reconcile(&mut self, seen: Outcome<Discovered<M>>) -> Drift<M> {
        match seen {
            Outcome::Found(found) => {
                let at = found.at_unix;
                let mut members = found.members.into_vec();
                members.sort();
                members.dedup();
                let attached: Vec<M> = self.attached.keys().cloned().collect();
                let orphans = missing_from(&members, &attached)
                    .into_iter()
                    .cloned()
                    .collect();
                let unobserved = missing_from(&attached, &members)
                    .into_iter()
                    .cloned()
                    .collect();
                for m in members {
                    self.discovered.entry(m).or_insert(at);
                }
                Drift {
                    orphans,
                    unobserved,
                    blind: None,
                }
            }
            Outcome::Empty { .. } => Drift {
                orphans: Vec::new(),
                unobserved: self.attached.keys().cloned().collect(),
                blind: None,
            },
            Outcome::Refused { because, .. } => Drift {
                orphans: Vec::new(),
                unobserved: Vec::new(),
                blind: Some(format!("refused: {because}")),
            },
            Outcome::Blind { because } => Drift {
                orphans: Vec::new(),
                unobserved: Vec::new(),
                blind: Some(format!("blind: {because}")),
            },
        }
    }

    /// The earliest presence on record for `member`, if it is owned.
    #[must_use]
    pub fn seen(&self, member: &M) -> Option<Seen<M>> {
        let attached = self.attached.get(member).copied();
        let discovered = self.discovered.get(member).copied();
        let at = match (attached, discovered) {
            (Some(a), Some(d)) => a.min(d),
            (Some(a), None) => a,
            (None, Some(d)) => d,
            (None, None) => return None,
        };
        Some(Seen::new(member.clone(), at))
    }

    /// Attached ∪ discovered, in order.
    ///
    /// # Panics
    ///
    /// Never: a footprint opens on a member and no method removes one.
    #[must_use]
    pub fn owned(&self) -> NonEmpty<M> {
        let mut all: Vec<M> = self.attached.keys().cloned().collect();
        all.extend(self.discovered.keys().cloned());
        all.sort();
        all.dedup();
        NonEmpty::from_vec(all).expect("a footprint opens on a member and never loses one")
    }

    /// The members recorded at create time.
    pub fn attached(&self) -> impl Iterator<Item = &M> {
        self.attached.keys()
    }

    /// The members a discovery probe has found.
    pub fn discovered(&self) -> impl Iterator<Item = &M> {
        self.discovered.keys()
    }

    /// The sole constructor of [`Reaped`]: every owned member must be covered
    /// by a proof of absence. Proofs about non-members are ignored; where a
    /// member has several proofs, the strongest admission stands.
    ///
    /// # Errors
    ///
    /// [`Outstanding`] names every owned member no proof covers.
    ///
    /// # Panics
    ///
    /// Never: the content-addressed record holds only strings and integers,
    /// which always have a canonical form.
    pub fn into_reaped<W: AbsenceWitness<Member = M>>(
        self,
        proofs: Vec<Absent<W>>,
    ) -> Result<Reaped<M>, Outstanding<M>> {
        let owned = self.owned();
        let mut best: BTreeMap<M, Absent<W>> = BTreeMap::new();
        for proof in proofs {
            match best.get(proof.member()) {
                Some(held) if held.admission().is_proof() || !proof.admission().is_proof() => {}
                _ => {
                    best.insert(proof.member().clone(), proof);
                }
            }
        }
        let proven: Vec<M> = best.keys().cloned().collect();
        let missing: Vec<M> = missing_from(owned.as_slice(), &proven)
            .into_iter()
            .cloned()
            .collect();
        if let Some(not_proven) = NonEmpty::from_vec(missing) {
            return Err(Outstanding { not_proven });
        }
        let used: Vec<&Absent<W>> = owned.iter().map(|m| &best[m]).collect();
        let tier = if used.iter().all(|p| p.admission().is_proof()) {
            Tier::Proven
        } else {
            Tier::OnlyMitigated
        };
        let record = ReapRecord {
            members: owned.iter().map(ToString::to_string).collect(),
            proofs: used
                .iter()
                .map(|p| ProofRecord {
                    member: p.member().to_string(),
                    evidence: match p.evidence() {
                        crate::Evidence::Witness(w) => w.to_string(),
                        crate::Evidence::Empty { of } => format!("empty: {of}"),
                    },
                    admission: match p.admission() {
                        Admission::Proof => "proof".to_owned(),
                        Admission::Mitigated(why) => format!("mitigated: {why}"),
                    },
                    present_seen_at: p.present_seen_at(),
                    absent_seen_at: p.absent_seen_at(),
                })
                .collect(),
            tier: tier.as_str(),
        };
        let digest = ContentAddress::of_json(&record)
            .expect("a record of strings and integers always has a canonical form");
        Ok(Reaped::new(owned, digest, tier))
    }
}

#[derive(Serialize)]
struct ReapRecord {
    members: Vec<String>,
    proofs: Vec<ProofRecord>,
    tier: &'static str,
}

#[derive(Serialize)]
struct ProofRecord {
    member: String,
    evidence: String,
    admission: String,
    present_seen_at: u64,
    absent_seen_at: u64,
}
