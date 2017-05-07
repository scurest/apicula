use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::Hash;
use std::string::ToString;

pub struct UniqueNamer {
    taken_names: HashSet<String>,
}

/// Returns unique names.
impl UniqueNamer {
    pub fn new() -> UniqueNamer {
        UniqueNamer { taken_names: HashSet::new() }
    }

    /// Returns a name, either `desired_name` or something "close" to it, which
    /// has never been returned by a prior call to this function on the same
    /// `UniqueNamer` receiver.
    pub fn get_fresh_name<S: AsRef<str>>(&mut self, desired_name: S) -> String {
        let desired_name = desired_name.as_ref();
        let chosen_name =
            if !self.taken_names.contains(desired_name) {
                desired_name.to_string()
            } else {
                let mut name = String::new();
                for i in 1.. {
                    name = format!("{}{}", desired_name, i);
                    if !self.taken_names.contains(&name) {
                        break;
                    }
                }
                name
            };
        self.taken_names.insert(chosen_name.clone());
        chosen_name
    }
}

/// A set which also assigns to each of its members a unique name.
pub struct UniqueNameSet<T> {
    names: HashMap<T, String>,
    unique_namer: UniqueNamer,
}

impl<T> UniqueNameSet<T>
where T: Hash + Eq + Clone + ToString {
    pub fn new() -> UniqueNameSet<T> {
        UniqueNameSet {
            names: HashMap::new(),
            unique_namer: UniqueNamer::new(),
        }
    }

    /// Insert `t` into the set, assigning it a unique name, which is
    /// returned.
    ///
    /// The assigned name is "close"" to `t.to_string()`, in the sense of
    /// `UniqueNamer`.
    pub fn get_name(&mut self, t: T) -> &str {
        match self.names.entry(t.clone()) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => {
                let desired_name = t.to_string();
                let name = self.unique_namer.get_fresh_name(desired_name);
                v.insert(name)
            }
        }
    }

    pub fn names_map(&self) -> &HashMap<T, String> {
        &self.names
    }
}

#[test]
fn test_unique_namer() {
    let mut un = UniqueNamer::new();
    assert_eq!(un.get_fresh_name("A"), "A");
    assert_eq!(un.get_fresh_name("A"), "A1");
    assert_eq!(un.get_fresh_name("A"), "A2");
    assert_eq!(un.get_fresh_name("B"), "B");
    assert_eq!(un.get_fresh_name("A"), "A3");
}


#[test]
fn test_unique_name_set() {
    #[derive(Clone, Hash, PartialEq, Eq)]
    struct S(i32, &'static str);
    impl ToString for S {
        fn to_string(&self) -> String { self.1.to_string() }
    }

    let mut set = UniqueNameSet::new();
    assert_eq!(set.get_name(S(0, "A")), "A");
    assert_eq!(set.get_name(S(0, "B")), "B");
    assert_eq!(set.get_name(S(1, "A")), "A1");
    assert_eq!(set.get_name(S(2, "A")), "A2");
    assert_eq!(set.get_name(S(0, "B")), "B");
}
