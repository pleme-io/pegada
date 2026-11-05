//! Kubernetes object identity, minted once.
//!
//! [`ObjectAddress`] is group, kind, scope and name: the identity of one
//! object across every version its kind is served under. [`Served`] adds the
//! version to fetch, [`Incarnation`] the uid of one life, [`Located`] the
//! cluster. Field order is identity order and is pinned by a test.
//!
//! The text form is `Kind[.group]/[namespace/]name`, and it parses back:
//!
//! ```
//! use pegada::kube::{ObjectAddress, Scope};
//! let a: ObjectAddress = "Deployment.apps/env-1/api".parse().unwrap();
//! assert_eq!(a.scope, Scope::Namespaced("env-1".parse().unwrap()));
//! assert_eq!(a.to_string(), "Deployment.apps/env-1/api");
//! let ns: ObjectAddress = "Namespace/env-1".parse().unwrap();
//! assert_eq!(ns.scope, Scope::Cluster);
//! ```

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::set::Member;

/// A component that does not satisfy its Kubernetes rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invalid {
    /// Which component.
    pub what: &'static str,
    /// The rejected text.
    pub value: String,
    /// The rule a legal value satisfies.
    pub legal: &'static str,
}

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid {} {:?}: {}", self.what, self.value, self.legal)
    }
}

impl std::error::Error for Invalid {}

fn dns_label(s: &str) -> bool {
    let b = s.as_bytes();
    (1..=63).contains(&b.len())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
        && b[0] != b'-'
        && b[b.len() - 1] != b'-'
}

fn group_rule(s: &str) -> bool {
    s.is_empty() || (s.len() <= 253 && s.split('.').all(dns_label))
}

fn kind_rule(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && s.chars().all(|c| c.is_ascii_alphanumeric())
}

fn name_rule(s: &str) -> bool {
    (1..=253).contains(&s.len())
        && s != "."
        && s != ".."
        && s.chars()
            .all(|c| c.is_ascii_graphic() && !matches!(c, '/' | '%' | '#' | '@'))
}

fn version_rule(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric())
}

fn uid_rule(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_graphic())
}

fn cluster_rule(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_graphic() && !matches!(c, ':' | '/'))
}

macro_rules! component {
    ($(#[$meta:meta])* $name:ident, $what:literal, $rule:ident, $legal:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            /// The text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = Invalid;

            fn try_from(value: String) -> Result<Self, Invalid> {
                if $rule(&value) {
                    Ok(Self(value))
                } else {
                    Err(Invalid { what: $what, value, legal: $legal })
                }
            }
        }

        impl FromStr for $name {
            type Err = Invalid;

            fn from_str(s: &str) -> Result<Self, Invalid> {
                Self::try_from(s.to_owned())
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> String {
                value.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

component!(
    /// An API group. The core group is empty: [`Group::core`].
    Group, "group", group_rule, "empty, or dot-separated DNS labels, at most 253 bytes"
);
component!(
    /// A kind, as `kind:` spells it.
    Kind, "kind", kind_rule, "ASCII alphanumeric, starting with a letter"
);
component!(
    /// A namespace name.
    Namespace, "namespace", dns_label, "a DNS label: 1-63 of [a-z0-9-], alphanumeric at both ends"
);
component!(
    /// An object name.
    Name, "name", name_rule, "1-253 printable ASCII bytes, no '/', '%', '#' or '@', not '.' or '..'"
);
component!(
    /// An API version, as served (`v1`, `v1beta1`).
    Version, "version", version_rule, "non-empty ASCII alphanumeric"
);
component!(
    /// The uid of one incarnation of an object.
    Uid, "uid", uid_rule, "non-empty printable ASCII"
);
component!(
    /// The cluster an object lives in.
    ClusterRef, "cluster", cluster_rule, "non-empty printable ASCII, no ':' or '/'"
);

impl Group {
    /// The core API group, which is empty.
    #[must_use]
    pub fn core() -> Self {
        Self(String::new())
    }

    /// Whether this is the core group.
    #[must_use]
    pub fn is_core(&self) -> bool {
        self.0.is_empty()
    }
}

/// A group and a kind: what an object is, independent of version.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GroupKind {
    /// The API group.
    pub group: Group,
    /// The kind.
    pub kind: Kind,
}

impl fmt::Display for GroupKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.group.is_core() {
            write!(f, "{}", self.kind)
        } else {
            write!(f, "{}.{}", self.kind, self.group)
        }
    }
}

impl FromStr for GroupKind {
    type Err = Invalid;

    fn from_str(s: &str) -> Result<Self, Invalid> {
        let (kind, group) = s.split_once('.').unwrap_or((s, ""));
        Ok(Self {
            group: group.parse()?,
            kind: kind.parse()?,
        })
    }
}

/// Where an object's name is unique.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Scope {
    /// Unique within a namespace.
    Namespaced(Namespace),
    /// Unique within the cluster.
    Cluster,
}

/// The identity of one object, version-insensitive.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ObjectAddress {
    /// What it is.
    pub gk: GroupKind,
    /// Where its name is unique.
    pub scope: Scope,
    /// Its name.
    pub name: Name,
}

impl fmt::Display for ObjectAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.scope {
            Scope::Namespaced(ns) => write!(f, "{}/{}/{}", self.gk, ns, self.name),
            Scope::Cluster => write!(f, "{}/{}", self.gk, self.name),
        }
    }
}

impl FromStr for ObjectAddress {
    type Err = Invalid;

    fn from_str(s: &str) -> Result<Self, Invalid> {
        let parts: Vec<&str> = s.split('/').collect();
        let (gk, scope, name) = match parts.as_slice() {
            [gk, name] => (gk, Scope::Cluster, name),
            [gk, ns, name] => (gk, Scope::Namespaced(ns.parse()?), name),
            _ => {
                return Err(Invalid {
                    what: "object address",
                    value: s.to_owned(),
                    legal: "Kind[.group]/[namespace/]name",
                });
            }
        };
        Ok(Self {
            gk: gk.parse()?,
            scope,
            name: name.parse()?,
        })
    }
}

/// An object address with the version it is fetched under.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Served {
    /// The object.
    pub address: ObjectAddress,
    /// The version to fetch it under.
    pub version: Version,
}

impl From<&Served> for ObjectAddress {
    fn from(served: &Served) -> Self {
        served.address.clone()
    }
}

impl From<Served> for ObjectAddress {
    fn from(served: Served) -> Self {
        served.address
    }
}

impl fmt::Display for Served {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.address, self.version)
    }
}

/// One life of an object: deleting and recreating it makes a new uid.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Incarnation {
    /// The object.
    pub address: ObjectAddress,
    /// This life's uid.
    pub uid: Uid,
}

impl fmt::Display for Incarnation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", self.address, self.uid)
    }
}

/// An item and the cluster it lives in.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Located<T> {
    /// The cluster.
    pub cluster: ClusterRef,
    /// The item.
    pub item: T,
}

impl<T: fmt::Display> fmt::Display for Located<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.cluster, self.item)
    }
}

impl Member for ObjectAddress {}
impl Member for Incarnation {}
impl<T: Member> Member for Located<T> {}
