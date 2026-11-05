//! Pegada — the footprint a lifecycle leaves where it stood, and the typed
//! proof that it is gone.
//!
//! A lifecycle that owns things in the world records each one at the moment
//! its creating call returns ([`Footprint::attach`]), cross-checks the record
//! with a differently shaped discovery probe ([`Footprint::reconcile`]), and
//! reaches its good terminal only by proving every owned member absent
//! ([`prove_absent`], then [`Footprint::into_reaped`]). A probe that could not
//! look, or was not understood, never counts as absence.
//!
//! ```
//! use kotae::Outcome;
//! use pegada::{absence_witness, prove_absent, Admission, Footprint, Sighting, Tier, kube::ObjectAddress};
//!
//! absence_witness! {
//!     /// What says a Kubernetes object is gone.
//!     pub enum KubeGone for ObjectAddress {
//!         empty => Admission::Proof;
//!         /// A GET answered 404.
//!         NotFound {} => Admission::Proof,
//!     }
//! }
//!
//! let ns: ObjectAddress = "Namespace/env-1".parse().unwrap();
//! let (footprint, attached) = Footprint::open(ns.clone(), 100);
//!
//! let blind = prove_absent::<KubeGone>(attached.seen(), Outcome::blind("vpn down"), 200);
//! assert!(blind.is_err());
//!
//! let gone = prove_absent(attached.seen(), Outcome::Found(Sighting::Gone(KubeGone::NotFound { member: ns })), 200).unwrap();
//! let reaped = footprint.into_reaped(vec![gone]).unwrap();
//! assert_eq!(reaped.tier(), Tier::Proven);
//! ```
//!
//! # Tier
//!
//! Omitting the proof is truly unrepresentable: [`Reaped`] has private fields
//! and one constructor, [`Absent`] has private fields and one constructor,
//! and a [`Footprint`] has no constructor from a list. What the proof's
//! content claims about the world (that a 404 really was a 404) is an external
//! fact, only mitigated, and [`Reaped::tier`] reports the weakest admission it
//! stands on. [`AbsenceWitness`] is sealed by a `#[doc(hidden)]` module that
//! [`absence_witness!`] implements; a hand implementation through that path is
//! possible and is hygiene, not a boundary.
//!
//! Reaping without proofs does not compile:
//!
//! ```compile_fail
//! use pegada::{Footprint, kube::ObjectAddress};
//! let a: ObjectAddress = "Namespace/env-1".parse().unwrap();
//! let (footprint, _) = Footprint::open(a, 1);
//! let _ = footprint.into_reaped();
//! ```
//!
//! Building an [`Absent`] by hand does not compile:
//!
//! ```compile_fail
//! use pegada::{absence_witness, Absent, Admission, Evidence, kube::ObjectAddress};
//! absence_witness! {
//!     pub enum KubeGone for ObjectAddress {
//!         empty => Admission::Proof;
//!         NotFound {} => Admission::Proof,
//!     }
//! }
//! let a: ObjectAddress = "Namespace/env-1".parse().unwrap();
//! let _: Absent<KubeGone> = Absent {
//!     member: a,
//!     evidence: Evidence::Empty { of: String::new() },
//!     admission: Admission::Proof,
//!     present_seen_at: 0,
//!     absent_seen_at: 1,
//! };
//! ```
//!
//! Proving absence without a prior presence does not compile:
//!
//! ```compile_fail
//! use kotae::Outcome;
//! use pegada::{absence_witness, prove_absent, Admission, Sighting, kube::ObjectAddress};
//! absence_witness! {
//!     pub enum KubeGone for ObjectAddress {
//!         empty => Admission::Proof;
//!         NotFound {} => Admission::Proof,
//!     }
//! }
//! let a: ObjectAddress = "Namespace/env-1".parse().unwrap();
//! let _ = prove_absent(a.clone(), Outcome::Found(Sighting::Gone(KubeGone::NotFound { member: a })), 1);
//! ```
//!
//! Pegada is Rust, not blue: its guarantees are compile-time seals a Rust
//! controller relies on, which a blue surface cannot give a Rust caller. A blue
//! surface over this crate is a later bidama.

pub mod absence;
pub mod footprint;
pub mod kube;
pub mod reaped;
pub mod set;

pub use absence::{
    AbsenceWitness, Absent, Admission, Evidence, Seen, Sighting, Unproven, prove_absent,
};
pub use footprint::{Attached, Discovered, Discovery, Drift, Footprint};
pub use reaped::{Outstanding, Reaped, Tier};
pub use set::{Member, NonEmpty, missing_from};
