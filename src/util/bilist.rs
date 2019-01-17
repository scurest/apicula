#![allow(dead_code)]

use std::slice;
use std::hash::Hash;
use std::collections::HashMap;

/// A list is a map {0, 1, ..., len - 1} -> T. A bilist is a bijective list.
///
/// Used as a list where you can look-up elements from their index or indices
/// from their element.
pub struct BiList<T: Eq + Hash + Clone> {
    list: Vec<T>,
    /// Maps each element to its index.
    reverse: HashMap<T, usize>,
}

pub type Iter<'a, T> = slice::Iter<'a, T>;

impl<T: Eq + Hash + Clone> BiList<T> {
    pub fn new() -> BiList<T> {
        BiList { list: vec![], reverse: HashMap::new() }
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn clear(&mut self) {
        self.list.clear();
        self.reverse.clear();
    }

    /// Push an element onto the list. Does nothing if the element is already in
    /// the list.
    pub fn push(&mut self, t: T) {
        let list = &mut self.list;
        self.reverse.entry(t.clone()).or_insert_with(|| {
            list.push(t);
            list.len() - 1
        });
    }

    pub fn get_elem(&self, idx: usize) -> Option<&T> {
        self.list.as_slice().get(idx)
    }

    pub fn index(&self, elem: &T) -> usize {
        self.reverse[elem]
    }

    pub fn as_slice(&self) -> &[T] {
        self.list.as_slice()
    }

    pub fn iter(&self) -> Iter<T> {
        self.list.iter()
    }
}

use std::ops::Index;

impl<T: Eq + Hash + Clone> Index<usize> for BiList<T> {
    type Output = T;
    fn index(&self, idx: usize) -> &T {
        &self.list[idx]
    }
}

#[test]
fn test() {
    let vals = [1, 0, 0, 1, 2, 3, 2, 1, 4, 0, 1i32];

    // Use a BiList to convert vals into a LUT+index representation.
    let mut lut = BiList::new();
    for &val in &vals {
        lut.push(val);
    }
    let indices = vals.iter()
        .map(|val| lut.index(val))
        .collect::<Vec<_>>();

    for i in 0..vals.len() {
        assert_eq!(vals[i], lut[indices[i]]);
    }
}
