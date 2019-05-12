use std::collections::HashMap;
use std::hash::Hash;

/// Bijective mapping between a set of Ks (on the left) and a set of Vs (on the
/// right).
pub struct BiMap<K, V>
where
    K: Hash + Eq + Clone,
    V: Hash + Eq + Clone,
{
    fwd: HashMap<K, V>,
    rev: HashMap<V, K>,
}

impl<K, V> BiMap<K, V>
where
    K: Hash + Eq + Clone,
    V: Hash + Eq + Clone,
{
    pub fn new() -> BiMap<K, V> {
        BiMap {
            fwd: HashMap::new(),
            rev: HashMap::new(),
        }
    }

    /// Go from a K to a V (from left to right).
    #[allow(dead_code)]
    pub fn forward(&self, k: &K) -> &V {
        &self.fwd[k]
    }

    /// Go from a V to a K (from right to left).
    pub fn backward(&self, v: &V) -> &K {
        &self.rev[v]
    }

    pub fn insert(&mut self, (k, v): (K, V)) {
        self.fwd.insert(k.clone(), v.clone());
        self.rev.insert(v, k);
    }

    /// Checks if the given V exists in the map.
    pub fn right_contains(&self, v: &V) -> bool {
        self.rev.contains_key(v)
    }

    pub fn iter(&self) -> std::collections::hash_map::Iter<K, V> {
        self.fwd.iter()
    }
}
