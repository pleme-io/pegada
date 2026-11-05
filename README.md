# pegada

**The footprint a lifecycle leaves where it stood, and the typed proof that it is gone.**

*Pegada* is Brazilian-Portuguese for *footprint*: the trace a thing leaves where it stood.

A lifecycle that creates things in the world (a namespace, a NodeClaim, a cloud
instance) does three things with pegada:

1. **It records what it owns when it creates it.** Each create response goes
   into `Footprint::attach`. A footprint has no constructor from a hand-written
   list, and it is never empty.
2. **It cross-checks the record with a second probe.** A label or tag sweep goes
   into `Footprint::reconcile` as a `kotae::Outcome`. `Drift::orphans` names
   what was created without a record. `Drift::unobserved` names what was
   recorded and cannot be seen. A blind or refused sweep changes nothing and
   says so.
3. **It reaches its good terminal only by proof.** `prove_absent` takes a prior
   `Seen` and a probe answer. Only an answered `Empty`, or a domain witness that
   says the member is gone, proves absence. `Blind` and `Refused` never count.
   `Footprint::into_reaped` is the only constructor of `Reaped`, and it returns
   `Outstanding` while any owned member is not proven gone.

```rust
use kotae::Outcome;
use pegada::{absence_witness, prove_absent, Admission, Footprint, Sighting, Tier, kube::ObjectAddress};

absence_witness! {
    /// What says a Kubernetes object is gone.
    pub enum KubeGone for ObjectAddress {
        empty => Admission::Proof;
        /// A GET answered 404.
        NotFound {} => Admission::Proof,
        /// A label-selected LIST omitted it.
        Unlisted {} => Admission::Mitigated("a list can lag"),
    }
}

let ns: ObjectAddress = "Namespace/env-1".parse()?;
let (footprint, attached) = Footprint::open(ns.clone(), now);
// ... delete, then read back:
let gone = prove_absent(attached.seen(), Outcome::Found(Sighting::Gone(KubeGone::NotFound { member: ns })), later)?;
let reaped = footprint.into_reaped(vec![gone])?;
assert_eq!(reaped.tier(), Tier::Proven);
```

## Modules

| Module | Holds |
|---|---|
| `set` | `NonEmpty<T>`, the `Member` trait, `missing_from` (set inclusion, written once) |
| `kube` | `ObjectAddress` (group, kind, scope, name; version-insensitive), `Served`, `Incarnation`, `Located<T>`, validated components |
| `footprint` | `Footprint`, `Attached`, `Discovered`, `Discovery`, `Drift` |
| `absence` | `AbsenceWitness`, `absence_witness!`, `Seen`, `Sighting`, `Absent`, `Evidence`, `Admission`, `Unproven`, `prove_absent` |
| `reaped` | `Reaped`, `Tier`, `Outstanding` |

## What is proven, and what is not

Omitting the proof does not compile: `Reaped`, `Absent`, `Seen` and `Footprint`
have private fields, and each has one constructor. `compile_fail` doctests pin
each of these. A witness arm without an `Admission` is a macro error.

What a proof claims about the world (that a 404 was a real 404) is an external
fact. Pegada cannot prove it. `Reaped::tier` reports the weakest admission the
reap stands on, so a reap built on a listing says `only-mitigated`.

`Reaped::digest` is a `tameshi` content address over the members and the
proofs.

## Why Rust

pleme-io writes new code in blue first, then tatara-lisp, then Rust. Pegada is
Rust because its guarantees are compile-time seals that a Rust controller
relies on, and a blue surface cannot give a Rust caller a seal. A blue surface
over this crate comes later, as a bidama.

## License

MIT
