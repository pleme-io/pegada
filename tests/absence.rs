use kotae::{Outcome, OutcomeKind};
use pegada::kube::ObjectAddress;
use pegada::{
    AbsenceWitness, Admission, Evidence, Footprint, Sighting, Tier, Unproven, absence_witness,
    prove_absent,
};

absence_witness! {
    pub enum KubeGone for ObjectAddress {
        empty => Admission::Mitigated("an empty list can lag");
        NotFound { status: u16 } => Admission::Proof,
        Unlisted {} => Admission::Mitigated("a list can lag"),
    }
}

fn addr(s: &str) -> ObjectAddress {
    s.parse().unwrap()
}

#[derive(Debug, PartialEq, Eq)]
enum Row {
    ProvesAbsent,
    Unproven(&'static str),
}

fn probe_of(kind: OutcomeKind, member: &ObjectAddress) -> Outcome<Sighting<KubeGone>> {
    match kind {
        OutcomeKind::Found => Outcome::Found(Sighting::Gone(KubeGone::NotFound {
            member: member.clone(),
            status: 404,
        })),
        OutcomeKind::Empty => Outcome::empty("namespace env-1"),
        OutcomeKind::Refused => Outcome::refused("bad selector", ["app=x"]),
        OutcomeKind::Blind => Outcome::blind("vpn down"),
    }
}

fn expected(kind: OutcomeKind) -> Row {
    match kind {
        OutcomeKind::Found | OutcomeKind::Empty => Row::ProvesAbsent,
        OutcomeKind::Refused => Row::Unproven("refused"),
        OutcomeKind::Blind => Row::Unproven("blind"),
    }
}

fn row_of(result: &Result<pegada::Absent<KubeGone>, Unproven>) -> Row {
    match result {
        Ok(_) => Row::ProvesAbsent,
        Err(Unproven::Blind(_)) => Row::Unproven("blind"),
        Err(Unproven::Refused(_)) => Row::Unproven("refused"),
        Err(Unproven::StillPresent) => Row::Unproven("still-present"),
        Err(Unproven::NeverSeenPresent) => Row::Unproven("never-seen-present"),
    }
}

#[test]
fn every_outcome_arm_has_a_prove_absent_row() {
    let m = addr("Namespace/env-1");
    let (fp, attached) = Footprint::open(m.clone(), 10);
    for kind in OutcomeKind::ALL {
        let got = prove_absent(attached.seen(), probe_of(*kind, &m), 20);
        assert_eq!(row_of(&got), expected(*kind), "arm {kind}");
    }
    assert_eq!(OutcomeKind::ALL.len(), 4);
    drop(fp);
}

#[test]
fn standing_is_still_present() {
    let m = addr("Namespace/env-1");
    let (_, attached) = Footprint::open(m, 10);
    let got = prove_absent::<KubeGone>(attached.seen(), Outcome::Found(Sighting::Standing), 20);
    assert_eq!(got.unwrap_err(), Unproven::StillPresent);
}

#[test]
fn a_witness_about_another_member_is_not_present_then_absent() {
    let (_, attached) = Footprint::open(addr("Namespace/env-1"), 10);
    let other = KubeGone::NotFound {
        member: addr("Namespace/env-2"),
        status: 404,
    };
    let got = prove_absent(attached.seen(), Outcome::Found(Sighting::Gone(other)), 20);
    assert_eq!(got.unwrap_err(), Unproven::NeverSeenPresent);
}

#[test]
fn absence_before_presence_is_not_present_then_absent() {
    let m = addr("Namespace/env-1");
    let (_, attached) = Footprint::open(m.clone(), 50);
    let got = prove_absent(attached.seen(), probe_of(OutcomeKind::Found, &m), 49);
    assert_eq!(got.unwrap_err(), Unproven::NeverSeenPresent);
}

#[test]
fn empty_takes_the_domains_empty_admission() {
    let m = addr("Namespace/env-1");
    let (_, attached) = Footprint::open(m, 10);
    let got = prove_absent::<KubeGone>(attached.seen(), Outcome::empty("ns"), 20).unwrap();
    assert_eq!(got.admission(), KubeGone::empty_admission());
    assert_eq!(
        got.evidence(),
        &Evidence::Empty {
            of: "ns".to_owned()
        }
    );
}

#[test]
fn every_witness_arm_states_its_admission() {
    let m = addr("Namespace/env-1");
    let arms = [
        KubeGone::NotFound {
            member: m.clone(),
            status: 404,
        },
        KubeGone::Unlisted { member: m.clone() },
    ];
    for arm in &arms {
        let want = match arm {
            KubeGone::NotFound { .. } => Admission::Proof,
            KubeGone::Unlisted { .. } => Admission::Mitigated("a list can lag"),
        };
        assert_eq!(arm.admission(), want);
        assert_eq!(arm.member(), &m);
    }
}

#[test]
fn admission_matrix() {
    for a in [Admission::Proof, Admission::Mitigated("x")] {
        let proof = match a {
            Admission::Proof => true,
            Admission::Mitigated(_) => false,
        };
        assert_eq!(a.is_proof(), proof);
    }
}

#[test]
fn unproven_matrix() {
    for u in [
        Unproven::NeverSeenPresent,
        Unproven::Blind("b".into()),
        Unproven::Refused("r".into()),
        Unproven::StillPresent,
    ] {
        let prefix = match &u {
            Unproven::NeverSeenPresent => "no presence",
            Unproven::Blind(_) => "blind:",
            Unproven::Refused(_) => "refused:",
            Unproven::StillPresent => "still present",
        };
        assert!(u.to_string().starts_with(prefix));
    }
}

#[test]
fn tier_matrix() {
    for t in Tier::ALL {
        let text = match t {
            Tier::Proven => "proven",
            Tier::OnlyMitigated => "only-mitigated",
        };
        assert_eq!(t.as_str(), text);
    }
    assert_eq!(Tier::ALL.len(), 2);
}

#[test]
fn a_mitigated_proof_makes_the_reap_only_mitigated() {
    let a = addr("Namespace/env-1");
    let b = addr("ClusterRole.rbac.authorization.k8s.io/env-1-reader");
    let (mut fp, sa) = Footprint::open(a.clone(), 1);
    let sb = fp.attach(b.clone(), 2);
    let pa = prove_absent(sa.seen(), probe_of(OutcomeKind::Found, &a), 5).unwrap();
    let pb = prove_absent(
        sb.seen(),
        Outcome::Found(Sighting::Gone(KubeGone::Unlisted { member: b })),
        5,
    )
    .unwrap();
    let reaped = fp.into_reaped(vec![pa, pb]).unwrap();
    assert_eq!(reaped.tier(), Tier::OnlyMitigated);
    assert_eq!(reaped.members().len(), 2);
}

#[test]
fn the_digest_is_stable_and_names_the_proofs() {
    let a = addr("Namespace/env-1");
    let build = |at: u64| {
        let (fp, s) = Footprint::open(a.clone(), 1);
        let p = prove_absent(s.seen(), probe_of(OutcomeKind::Found, &a), at).unwrap();
        fp.into_reaped(vec![p]).unwrap().digest()
    };
    assert_eq!(build(5), build(5));
    assert_ne!(build(5), build(6));
    assert!(build(5).to_b3().starts_with("b3:"));
}
