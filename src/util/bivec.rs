use std;
use std::collections::HashMap;
use std::hash::Hash;

/// If a Vec is a map {0,1,...,n} -> T, a BiVec is a bijective Vec.
/// Can lookup elements by index, or indices by element.
pub struct BiVec<T>
where
    T: Clone + Eq + Hash,
{
    vec: Vec<T>,
    reverse: HashMap<T, usize>, // vec[reverse[x]] == x
}

impl<T> BiVec<T>
where
    T: Clone + Eq + Hash,
{
    pub fn new() -> BiVec<T> {
        BiVec {
            vec: vec![],
            reverse: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.vec.len()
    }

    pub fn clear(&mut self) {
        self.vec.clear();
        self.reverse.clear();
    }

    /// Push an element to the vec (if not already in it).
    /// Returns the index of the element.
    pub fn push(&mut self, x: T) -> usize {
        let vec = &mut self.vec;
        *self.reverse
            .entry(x.clone())
            .or_insert_with(|| {
                vec.push(x);
                vec.len() - 1
            })
    }

    pub fn idx(&self, x: &T) -> usize {
        self.reverse[x]
    }

    pub fn iter(&self) -> std::slice::Iter<T> {
        self.vec.iter()
    }
}

impl<T> std::ops::Index<usize> for BiVec<T>
where
    T: Clone + Eq + Hash,
{
    type Output = T;
    fn index(&self, idx: usize) -> &T {
        &self.vec[idx]
    }
}

#[test]
fn lut_test() {
    let xs = [12, 0, 12, 1, 4, 7, -2, 3, 7];

    let mut lut = BiVec::new();
    for &x in &xs {
        lut.push(x);
    }

    for &x in &xs {
        assert_eq!(lut[lut.idx(&x)], x);
    }
}
