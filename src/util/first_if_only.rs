/// If `it` yields a single value, return that value. Otherwise, return `None`.
pub fn first_if_only<I: Iterator>(mut it: I) -> Option<<I as Iterator>::Item> {
    let first = match it.next() {
        Some(x) => x,
        None => return None,
    };
    match it.next() {
        Some(_) => None,
        None => Some(first),
    }
}
