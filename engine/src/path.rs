//! Dependency path queries.

use std::collections::{HashMap, VecDeque};

use crate::layout::Link;

/// Shortest dependency path, by hop count: "three steps away" is actionable
/// where a weighted cost is not.
pub(crate) fn shortest_path(links: &[Link], from: &str, to: &str) -> Vec<String> {
    if from == to {
        return vec![from.to_string()];
    }
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for lr in links {
        adj.entry(lr.source.as_str())
            .or_default()
            .push(lr.target.as_str());
        adj.entry(lr.target.as_str())
            .or_default()
            .push(lr.source.as_str());
    }

    let mut prev: HashMap<&str, &str> = HashMap::new();
    let mut queue = VecDeque::new();
    queue.push_back(from);
    prev.insert(from, from);

    while let Some(cur) = queue.pop_front() {
        if cur == to {
            let mut path = vec![cur.to_string()];
            let mut at = cur;
            while prev[at] != at {
                at = prev[at];
                path.push(at.to_string());
            }
            path.reverse();
            return path;
        }
        for &next in adj.get(cur).into_iter().flatten() {
            if !prev.contains_key(next) {
                prev.insert(next, cur);
                queue.push_back(next);
            }
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn links(pairs: &[(&str, &str)]) -> Vec<Link> {
        pairs
            .iter()
            .map(|(a, b)| Link {
                source: a.to_string(),
                target: b.to_string(),
                ..Default::default()
            })
            .collect()
    }

    #[test]
    fn finds_the_shortest_route_not_merely_a_route() {
        // A long way round exists; BFS must take the short one.
        let l = links(&[("a", "b"), ("b", "c"), ("c", "d"), ("a", "x"), ("x", "d")]);
        assert_eq!(shortest_path(&l, "a", "d"), vec!["a", "x", "d"]);
    }

    #[test]
    fn edges_are_followed_in_both_directions() {
        // Links are stored canonically, so direction must not matter.
        let l = links(&[("b", "a"), ("c", "b")]);
        assert_eq!(shortest_path(&l, "a", "c"), vec!["a", "b", "c"]);
    }

    #[test]
    fn disconnected_pair_yields_nothing() {
        let l = links(&[("a", "b"), ("c", "d")]);
        assert!(shortest_path(&l, "a", "d").is_empty());
    }

    #[test]
    fn a_building_reaches_itself_in_zero_hops() {
        let l = links(&[("a", "b")]);
        assert_eq!(shortest_path(&l, "a", "a"), vec!["a"]);
    }

    #[test]
    fn unknown_endpoint_is_not_a_panic() {
        let l = links(&[("a", "b")]);
        assert!(shortest_path(&l, "a", "ghost").is_empty());
        assert!(shortest_path(&l, "ghost", "a").is_empty());
    }
}
