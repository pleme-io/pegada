use kotae::Outcome;
use pegada::kube::{
    ClusterRef, Group, GroupKind, Incarnation, Kind, Located, Name, Namespace, ObjectAddress,
    Scope, Served, Uid, Version,
};
use pegada::{Discovered, Discovery, Footprint, NonEmpty};

fn addr(s: &str) -> ObjectAddress {
    s.parse().unwrap()
}

#[test]
fn field_order_is_identity_order() {
    let mut v = [
        addr("Secret/b/x"),
        addr("Deployment.apps/a/z"),
        addr("ConfigMap/b/a"),
        addr("ConfigMap/a/z"),
        addr("Namespace/a"),
        addr("ClusterRole.rbac.authorization.k8s.io/a"),
        addr("ConfigMap/a/a"),
    ];
    v.sort();
    let got: Vec<String> = v.iter().map(ToString::to_string).collect();
    assert_eq!(
        got,
        [
            "ConfigMap/a/a",
            "ConfigMap/a/z",
            "ConfigMap/b/a",
            "Namespace/a",
            "Secret/b/x",
            "Deployment.apps/a/z",
            "ClusterRole.rbac.authorization.k8s.io/a",
        ]
    );
}

#[test]
fn scope_matrix() {
    for scope in [Scope::Namespaced("env-1".parse().unwrap()), Scope::Cluster] {
        let a = ObjectAddress {
            gk: "ConfigMap".parse().unwrap(),
            scope: scope.clone(),
            name: "x".parse().unwrap(),
        };
        let text = match &scope {
            Scope::Namespaced(_) => "ConfigMap/env-1/x",
            Scope::Cluster => "ConfigMap/x",
        };
        assert_eq!(a.to_string(), text);
        assert_eq!(addr(text), a);
    }
}

#[test]
fn core_group_is_spelled_once() {
    assert!(Group::core().is_core());
    assert_eq!(Group::core(), "".parse::<Group>().unwrap());
    let gk: GroupKind = "Namespace".parse().unwrap();
    assert_eq!(gk.group, Group::core());
}

#[test]
fn illegal_components_are_refused() {
    assert!("".parse::<Kind>().is_err());
    assert!("Config.Map".parse::<Kind>().is_err());
    assert!("Env_1".parse::<Namespace>().is_err());
    assert!("-env".parse::<Namespace>().is_err());
    assert!("a".repeat(64).parse::<Namespace>().is_err());
    assert!("a/b".parse::<Name>().is_err());
    assert!("a#b".parse::<Name>().is_err());
    assert!("..".parse::<Name>().is_err());
    assert!("".parse::<Version>().is_err());
    assert!("a b".parse::<Uid>().is_err());
    assert!("a:b".parse::<ClusterRef>().is_err());
    assert!("Apps".parse::<Group>().is_err());
    assert!("ConfigMap/a/b/c".parse::<ObjectAddress>().is_err());
    assert!("ConfigMap".parse::<ObjectAddress>().is_err());
}

#[test]
fn serde_validates_on_the_way_in() {
    let a = addr("Deployment.apps/env-1/api");
    let json = serde_json::to_string(&a).unwrap();
    let back: ObjectAddress = serde_json::from_str(&json).unwrap();
    assert_eq!(back, a);
    let bad = json.replace("env-1", "Env_1");
    assert!(serde_json::from_str::<ObjectAddress>(&bad).is_err());
}

#[test]
fn precisions_display_distinctly() {
    let a = addr("Namespace/env-1");
    let served = Served {
        address: a.clone(),
        version: "v1".parse().unwrap(),
    };
    let inc = Incarnation {
        address: a.clone(),
        uid: "u-1".parse().unwrap(),
    };
    let loc = Located {
        cluster: "ryn".parse().unwrap(),
        item: inc.clone(),
    };
    assert_eq!(served.to_string(), "Namespace/env-1@v1");
    assert_eq!(inc.to_string(), "Namespace/env-1#u-1");
    assert_eq!(loc.to_string(), "ryn:Namespace/env-1#u-1");
}

#[test]
fn a_located_incarnation_is_a_member() {
    let inc = |uid: &str| Located {
        cluster: "ryn".parse::<ClusterRef>().unwrap(),
        item: Incarnation {
            address: addr("Namespace/env-1"),
            uid: uid.parse().unwrap(),
        },
    };
    let (mut fp, _) = Footprint::open(inc("u-1"), 1);
    fp.attach(inc("u-2"), 2);
    assert_eq!(fp.owned().len(), 2);
}

#[test]
fn attach_twice_is_one_member_at_its_first_time() {
    let a = addr("Namespace/env-1");
    let (mut fp, _) = Footprint::open(a.clone(), 5);
    fp.attach(a.clone(), 9);
    assert_eq!(fp.owned().len(), 1);
    assert_eq!(fp.seen(&a).unwrap().at_unix(), 5);
}

#[test]
fn a_footprint_opened_by_discovery_owns_what_it_found() {
    let a = addr("Namespace/env-1");
    let fp = Footprint::discover(Discovered::new(
        NonEmpty::new(a.clone(), []),
        Discovery::new("app.kubernetes.io/managed-by=viveiro"),
        3,
    ));
    assert_eq!(fp.attached().count(), 0);
    assert_eq!(fp.owned().into_vec(), vec![a.clone()]);
    assert_eq!(fp.seen(&a).unwrap().at_unix(), 3);
    assert!(fp.seen(&addr("Namespace/other")).is_none());
}

#[test]
fn an_empty_find_leaves_every_attached_member_unobserved() {
    let a = addr("Namespace/env-1");
    let (mut fp, _) = Footprint::open(a.clone(), 1);
    let drift = fp.reconcile(Outcome::empty("objects labelled env-1"));
    assert_eq!(drift.unobserved, vec![a]);
    assert_eq!(drift.orphans, Vec::<ObjectAddress>::new());
    assert!(drift.blind.is_none());
}
