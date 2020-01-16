use std;

/// A tree (or forest). Each node stores a T.
/// Nodes are stored in a flat array and referenced by indices.
pub struct Tree<T> {
    nodes: Vec<Node<T>>,
}

pub type NodeIdx = u16;
static NONE: NodeIdx = !0; // marks no children/siblings

struct Node<T> {
    val: T,
    parent: NodeIdx,
    first_child: NodeIdx,
    next_sibling: NodeIdx,
    prev_sibling: NodeIdx,
}

impl<T> Tree<T> {
    pub fn with_capacity(num_nodes: usize) -> Tree<T> {
        Tree {
            nodes: Vec::with_capacity(num_nodes),
        }
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn add_node(&mut self, val: T) -> NodeIdx {
        self.nodes.push(Node {
            val,
            parent: NONE,
            first_child: NONE,
            next_sibling: NONE,
            prev_sibling: NONE,
        });
        let node = (self.nodes.len() - 1) as NodeIdx;
        assert!(node != NONE);
        node
    }

    pub fn reparent(&mut self, node: NodeIdx, new_parent: NodeIdx) {
        // Remove from current position
        let parent = self.nodes[node as usize].parent;
        let prev_sibling = self.nodes[node as usize].prev_sibling;
        let next_sibling = self.nodes[node as usize].next_sibling;
        if prev_sibling != NONE {
            self.nodes[prev_sibling as usize].next_sibling = next_sibling;
        }
        if next_sibling != NONE {
            self.nodes[next_sibling as usize].prev_sibling = prev_sibling;
        }
        if parent != NONE && prev_sibling == NONE {
            assert!(self.nodes[parent as usize].first_child == node);
            self.nodes[parent as usize].first_child = next_sibling;
        }

        // Insert at new position (as last child)
        let last_child = self.last_child(new_parent);
        if last_child == NONE {
            self.nodes[new_parent as usize].first_child = node;
        } else {
            self.nodes[last_child as usize].next_sibling = node;
        }
        self.nodes[node as usize].prev_sibling = last_child;
        self.nodes[node as usize].next_sibling = NONE;
        self.nodes[node as usize].parent = new_parent;
    }

    fn last_child(&self, node: NodeIdx) -> NodeIdx {
        let mut child = self.nodes[node as usize].first_child;
        if child == NONE {
            return NONE;
        }

        loop {
            let next_child = self.nodes[child as usize].next_sibling;
            if next_child == NONE {
                return child;
            } else {
                child = next_child;
            }
        }
    }

    // Iterator over all nodes indices (in insertion order).
    pub fn node_idxs(&self) -> std::ops::Range<u16> {
        0..self.nodes.len() as NodeIdx
    }

    /// Iterator over all the children of node.
    pub fn children(&self, node: NodeIdx) -> Children<T> {
        Children {
            tree: self,
            next_child: self.nodes[node as usize].first_child,
        }
    }
}

impl<T> std::ops::Index<NodeIdx> for Tree<T> {
    type Output = T;

    fn index(&self, node: NodeIdx) -> &T {
        &self.nodes[node as usize].val
    }
}

impl<T> std::ops::IndexMut<NodeIdx> for Tree<T> {
    fn index_mut(&mut self, node: NodeIdx) -> &mut T {
        &mut self.nodes[node as usize].val
    }
}

pub struct Children<'a, T> {
    tree: &'a Tree<T>,
    next_child: NodeIdx,
}

impl<T> std::iter::Iterator for Children<'_, T> {
    type Item = NodeIdx;

    fn next(&mut self) -> Option<NodeIdx> {
        if self.next_child == NONE {
            None
        } else {
            let next_child = self.next_child;
            self.next_child = self.tree.nodes[self.next_child as usize].next_sibling;
            Some(next_child)
        }
    }
}
