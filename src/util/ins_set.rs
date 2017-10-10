#![allow(dead_code)]

use std::slice;
use std::borrow::Borrow;
use std::hash::Hash;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

/// A set of `T`s, together with a bijective mapping from its elements
/// to the order each was inserted into the set.
///
/// This can be used for the situation where
///
/// 1. you have a value
/// 2. you need an index into a global list of values
///
/// In that case, you can first insert all the value into an `InsOrderSet`
/// and build the global list by iterating over this set. Then to get the
/// index given a value, you just use `get_index_from_value`.
///
/// An example is a string table.
pub struct InsOrderSet<T: Eq + Hash + Clone> {
    /// All the items in the set in insertion order.
    vec: Vec<T>,
    /// Maps from a T to its position in `vec`.
    map: HashMap<T, usize>,
}

pub type Iter<'a, T: 'a> = slice::Iter<'a, T>;

impl<T: Eq + Hash + Clone> InsOrderSet<T> {
    /// Create an empty set.
    pub fn new() -> InsOrderSet<T> {
        InsOrderSet {
            vec: vec![],
            map: HashMap::new()
        }
    }

    /// The number of elements in the set.
    pub fn len(&self) -> usize {
        self.vec.len()
    }

    /// Clear all entries from the set.
    pub fn clear(&mut self) {
        self.vec.clear();
        self.map.clear();
    }

    /// Insert an element into the set. This operation does nothing
    /// if the set already contained the element.
    pub fn insert(&mut self, t: T) {
        let entry = self.map.entry(t.clone());
        match entry {
            Entry::Occupied(_) => (),
            Entry::Vacant(v) => {
                let index = self.vec.len();
                self.vec.push(t);
                v.insert(index);
            }
        }
    }

    pub fn get_value_from_index(&self, idx: usize) -> Option<&T> {
        self.vec.as_slice().get(idx)
    }

    pub fn get_index_from_value<Q: ?Sized>(&self, k: &Q) -> Option<usize>
    where T: Borrow<Q>, Q: Hash + Eq {
        self.map.get(k).cloned()
    }

    /// Iterate over the set in insertion order.
    pub fn iter(&self) -> Iter<T> {
        self.vec.iter()
    }
}

#[test]
fn test1() {
    let mut s = InsOrderSet::<i32>::new();

    s.insert(0);
    s.insert(1);
    s.insert(1);
    s.insert(2);
    s.insert(1);

    assert_eq!(s.len(), 3);
    assert_eq!(s.get_index_from_value(&1).unwrap(), 1);
}

#[test]
fn test2() {
    let vals = [1, 0, 0, 1, 2, 3, 2, 1, 4, 0, 1i32];

    let mut s = InsOrderSet::new();
    for &val in &vals {
        s.insert(val);
    }

    // List of all values
    let table = s.iter().cloned().collect::<Vec<_>>();
    assert_eq!(&table, &[1,0,2,3,4]);

    // Same as `vals`, but stores each element by its index into `table`.
    let vals_by_index = vals.iter()
        .map(|val| s.get_index_from_value(val).unwrap().clone())
        .collect::<Vec<_>>();
    assert_eq!(&vals_by_index, &[0, 1, 1, 0, 2, 3, 2, 0, 4, 1, 0]);

    // Check that each element is indeed the index of the corresponding
    // element of `val` in `table`.
    for (&val, &index) in vals.iter().zip(vals_by_index.iter()) {
        assert_eq!(val, table[index]);
    }
}
