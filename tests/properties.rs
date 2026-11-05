use std::collections::BTreeSet;

use kotae::Outcome;
use pegada::kube::{GroupKind, Name, ObjectAddress, Scope, Served};
use pegada::{
    Admission, Discovered, Discovery, Footprint, NonEmpty, Sighting, Tier, absence_witness,
    missing_from, prove_absent,
};
use proptest::prelude::*;

absence_witness! {
    pub enum Gone for ObjectAddress {
        empty => Admission::Proof;
        NotFound {} => Admission::Proof,
        Unlisted {} => Admission::Mitigated("a list can lag"),
    }
}

fn address() -> impl Strategy<Value = ObjectAddress> {
    (
        prop::sample::select(vec!["", "apps", "rbac.authorization.k8s.io", "batch"]),
        prop::sample::select(vec!["Namespace", "ConfigMap", "Deployment", "ClusterRole"]),
        prop::option::of("[a-z][a-z0-9-]{0,8}[a-z0-9]"),
        "[a-z0-9][a-z0-9.:_-]{0,12}",
    )
        .prop_filter_map("valid", |(g, k, ns, n)| {
            Some(ObjectAddress {
                gk: GroupKind {
                    group: g.parse().ok()?,
                    kind: k.parse().ok()?,
                },
                scope: match ns {
                    Some(ns) => Scope::Namespaced(ns.parse().ok()?),
                    None => Scope::Cluster,
                },
                name: n.parse::<Name>().ok()?,
            })
        })
}

proptest! {
    #[test]
    fn missing_from_is_set_difference(expected in prop::collection::vec(0u8..20, 0..15), actual in prop::collection::vec(0u8..20, 0..15)) {
        let got: Vec<u8> = missing_from(&expected, &actual).into_iter().copied().collect();
        let actual_set: BTreeSet<u8> = actual.iter().copied().collect();
        let want: Vec<u8> = expected.iter().copied().filter(|x| !actual_set.contains(x)).collect();
        prop_assert_eq!(got, want);
    }

    #[test]
    fn missing_from_a_superset_is_empty(expected in prop::collection::vec(0u8..20, 0..15), extra in prop::collection::vec(0u8..20, 0..5)) {
        let mut actual = expected.clone();
        actual.extend(extra);
        prop_assert!(missing_from(&expected, &actual).is_empty());
    }

    #[test]
    fn into_reaped_is_ok_iff_proofs_cover_attached_and_discovered(
        attached in prop::collection::btree_set(address(), 1..6),
        discovered in prop::collection::btree_set(address(), 0..6),
        mask in prop::collection::vec(any::<bool>(), 12),
        mitigated in prop::collection::vec(any::<bool>(), 12),
    ) {
        let attached: Vec<ObjectAddress> = attached.into_iter().collect();
        let (mut fp, _) = Footprint::open(attached[0].clone(), 1);
        for a in &attached[1..] {
            fp.attach(a.clone(), 1);
        }
        if let Some(found) = NonEmpty::from_vec(discovered.iter().cloned().collect()) {
            fp.reconcile(Outcome::Found(Discovered::new(found, Discovery::new("label"), 2)));
        }
        let owned: BTreeSet<ObjectAddress> = attached.iter().cloned().chain(discovered.iter().cloned()).collect();
        prop_assert_eq!(fp.owned().into_vec(), owned.iter().cloned().collect::<Vec<_>>());

        let mut proofs = Vec::new();
        let mut covered = BTreeSet::new();
        let mut any_mitigated = false;
        for (i, m) in owned.iter().enumerate() {
            if mask[i] {
                let w = if mitigated[i] { Gone::Unlisted { member: m.clone() } } else { Gone::NotFound { member: m.clone() } };
                any_mitigated |= mitigated[i];
                let seen = fp.seen(m).unwrap();
                proofs.push(prove_absent(seen, Outcome::Found(Sighting::Gone(w)), 10).unwrap());
                covered.insert(m.clone());
            }
        }
        let covers = covered == owned;
        match fp.into_reaped(proofs) {
            Ok(r) => {
                prop_assert!(covers);
                prop_assert_eq!(r.tier(), if any_mitigated { Tier::OnlyMitigated } else { Tier::Proven });
            }
            Err(out) => {
                prop_assert!(!covers);
                let gap: Vec<ObjectAddress> = owned.difference(&covered).cloned().collect();
                prop_assert_eq!(out.not_proven.into_vec(), gap);
            }
        }
    }

    #[test]
    fn reconcile_partitions_the_find(
        attached in prop::collection::btree_set(address(), 1..6),
        found in prop::collection::btree_set(address(), 1..6),
    ) {
        let attached: Vec<ObjectAddress> = attached.into_iter().collect();
        let (mut fp, _) = Footprint::open(attached[0].clone(), 1);
        for a in &attached[1..] {
            fp.attach(a.clone(), 1);
        }
        let found_vec: Vec<ObjectAddress> = found.iter().cloned().collect();
        let drift = fp.reconcile(Outcome::Found(Discovered::new(NonEmpty::from_vec(found_vec).unwrap(), Discovery::new("label"), 2)));
        let att: BTreeSet<_> = attached.iter().cloned().collect();
        let orphans: BTreeSet<_> = drift.orphans.iter().cloned().collect();
        let unobserved: BTreeSet<_> = drift.unobserved.iter().cloned().collect();
        prop_assert_eq!(&orphans, &found.difference(&att).cloned().collect());
        prop_assert_eq!(&unobserved, &att.difference(&found).cloned().collect());
        prop_assert!(drift.blind.is_none());
    }

    #[test]
    fn a_probe_that_did_not_answer_changes_nothing(
        attached in prop::collection::btree_set(address(), 1..6),
        refused in any::<bool>(),
    ) {
        let attached: Vec<ObjectAddress> = attached.into_iter().collect();
        let (mut fp, _) = Footprint::open(attached[0].clone(), 1);
        for a in &attached[1..] {
            fp.attach(a.clone(), 1);
        }
        let before = fp.clone();
        let answer = if refused { Outcome::refused("bad selector", ["app=x"]) } else { Outcome::blind("vpn down") };
        let drift = fp.reconcile(answer);
        prop_assert_eq!(&fp, &before);
        prop_assert!(drift.orphans.is_empty() && drift.unobserved.is_empty());
        prop_assert!(drift.blind.is_some());
    }

    #[test]
    fn served_to_address_round_trips_through_text(a in address(), v in "v[1-9](beta[1-9])?") {
        let served = Served { address: a, version: v.parse().unwrap() };
        let address = ObjectAddress::from(&served);
        let parsed: ObjectAddress = address.to_string().parse().unwrap();
        prop_assert_eq!(parsed, address);
    }

    #[test]
    fn address_identity_ignores_version(a in address(), v1 in "v[1-9]", v2 in "v[1-9]beta[1-9]") {
        let s1 = Served { address: a.clone(), version: v1.parse().unwrap() };
        let s2 = Served { address: a, version: v2.parse().unwrap() };
        prop_assert_ne!(&s1, &s2);
        prop_assert_eq!(ObjectAddress::from(&s1).cmp(&ObjectAddress::from(&s2)), std::cmp::Ordering::Equal);
    }
}
