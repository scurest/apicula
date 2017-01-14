use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::Hash;
use std::string::ToString;

pub struct UniqueNamer<T> {
    map_: HashMap<T, String>,
    taken_names: HashSet<String>,
}

impl<T> UniqueNamer<T>
where T: Hash + Eq + Clone + ToString {
    pub fn new() -> UniqueNamer<T> {
        UniqueNamer {
            map_: HashMap::new(),
            taken_names: HashSet::new(),
        }
    }

    pub fn get_name(&mut self, t: T) -> &str {
        match self.map_.entry(t.clone()) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => {
                let desired_name = t.to_string();
                let name = find_free_name(&mut self.taken_names, desired_name);
                v.insert(name)
            }
        }
    }

    pub fn map(&self) -> &HashMap<T, String> {
        &self.map_
    }
}

/// Picks the first name from
///
/// * `"{desired_name}"`
/// * `"{desired_name}1"`
/// * `"{desired_name}2"`
/// * ...
///
/// which is not a taken name, marks it as taken, and returns it.
fn find_free_name(taken_names: &mut HashSet<String>, desired_name: String) -> String {
    let chosen_name = if !taken_names.contains(&desired_name) {
        desired_name
    } else {
        let mut name = String::new();
        for i in 1.. {
            name = format!("{}{}", desired_name, i);
            if !taken_names.contains(&name) {
                break;
            }
        }
        name
    };
    taken_names.insert(chosen_name.clone());
    chosen_name
}

#[test]
fn test() {
    #[derive(Clone, Hash, PartialEq, Eq)]
    struct S(i32, &'static str);
    impl ToString for S {
        fn to_string(&self) -> String { self.1.to_string() }
    }

    let mut un = UniqueNamer::new();
    assert_eq!(un.get_name(S(0, "A")), "A");
    assert_eq!(un.get_name(S(0, "B")), "B");
    assert_eq!(un.get_name(S(1, "A")), "A1");
    assert_eq!(un.get_name(S(2, "A")), "A2");
    assert_eq!(un.get_name(S(0, "B")), "B");
}
