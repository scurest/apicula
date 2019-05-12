use std::collections::HashSet;
use std::string::ToString;

pub struct UniqueNamer {
    taken_names: HashSet<String>,
}

/// Returns unique names.
impl UniqueNamer {
    pub fn new() -> UniqueNamer {
        UniqueNamer {
            taken_names: HashSet::new(),
        }
    }

    /// Returns a name, either `desired_name` or something "close" to it, which
    /// has never been returned by a prior call to this function on the same
    /// `UniqueNamer` receiver.
    pub fn get_fresh_name<S: AsRef<str>>(&mut self, desired_name: S) -> String {
        let desired_name = desired_name.as_ref();
        let chosen_name = if !self.taken_names.contains(desired_name) {
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

#[test]
fn test_unique_namer() {
    let mut un = UniqueNamer::new();
    assert_eq!(un.get_fresh_name("A"), "A");
    assert_eq!(un.get_fresh_name("A"), "A1");
    assert_eq!(un.get_fresh_name("A"), "A2");
    assert_eq!(un.get_fresh_name("B"), "B");
    assert_eq!(un.get_fresh_name("A"), "A3");
}
