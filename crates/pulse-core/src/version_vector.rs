use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// How two versions of the same file relate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionOrdering {
    Equal,
    /// `self` has seen everything `other` has, and more.
    Newer,
    /// `other` has seen everything `self` has, and more.
    Older,
    /// Both sides changed independently: a conflict.
    Concurrent,
}

/// A version vector: for each device (by short id), how many changes it has
/// made to this file. Used to tell "newer" apart from "concurrent edit".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VersionVector(BTreeMap<u64, u64>);

impl VersionVector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, device: u64) -> u64 {
        self.0.get(&device).copied().unwrap_or(0)
    }

    /// Records a local change made by `device`.
    pub fn bump(&mut self, device: u64) {
        *self.0.entry(device).or_insert(0) += 1;
    }

    /// Pointwise maximum: the version that has seen both histories.
    pub fn merge(&mut self, other: &VersionVector) {
        for (&dev, &counter) in &other.0 {
            let entry = self.0.entry(dev).or_insert(0);
            *entry = (*entry).max(counter);
        }
    }

    pub fn compare(&self, other: &VersionVector) -> VersionOrdering {
        let mut ordering = Ordering::Equal;
        let devices = self.0.keys().chain(other.0.keys());
        for &dev in devices {
            match (self.get(dev).cmp(&other.get(dev)), ordering) {
                (Ordering::Equal, _) => {}
                (o, Ordering::Equal) => ordering = o,
                (o, current) if o != current => return VersionOrdering::Concurrent,
                _ => {}
            }
        }
        match ordering {
            Ordering::Equal => VersionOrdering::Equal,
            Ordering::Greater => VersionOrdering::Newer,
            Ordering::Less => VersionOrdering::Older,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: u64 = 1;
    const B: u64 = 2;

    fn vv(entries: &[(u64, u64)]) -> VersionVector {
        VersionVector(entries.iter().copied().collect())
    }

    #[test]
    fn compare_covers_all_cases() {
        assert_eq!(vv(&[]).compare(&vv(&[])), VersionOrdering::Equal);
        assert_eq!(
            vv(&[(A, 2)]).compare(&vv(&[(A, 1)])),
            VersionOrdering::Newer
        );
        assert_eq!(
            vv(&[(A, 1)]).compare(&vv(&[(A, 1), (B, 1)])),
            VersionOrdering::Older
        );
        assert_eq!(
            vv(&[(A, 2)]).compare(&vv(&[(A, 1), (B, 1)])),
            VersionOrdering::Concurrent
        );
    }

    #[test]
    fn merge_then_bump_dominates_both() {
        let left = vv(&[(A, 2)]);
        let right = vv(&[(A, 1), (B, 3)]);
        let mut merged = left.clone();
        merged.merge(&right);
        merged.bump(A);
        assert_eq!(merged.compare(&left), VersionOrdering::Newer);
        assert_eq!(merged.compare(&right), VersionOrdering::Newer);
    }

    #[test]
    fn serializes_as_json_object() {
        let json = serde_json::to_string(&vv(&[(A, 3)])).unwrap();
        assert_eq!(json, r#"{"1":3}"#);
        let back: VersionVector = serde_json::from_str(&json).unwrap();
        assert_eq!(back, vv(&[(A, 3)]));
    }
}
