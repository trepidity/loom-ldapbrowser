use crate::dn;

/// A node in the directory tree.
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub dn: String,
    pub display_name: String,
    pub children: Option<Vec<TreeNode>>,
    pub has_children_hint: bool,
}

impl TreeNode {
    pub fn new(dn: String) -> Self {
        let display_name = dn::rdn_display_name(&dn).to_string();
        Self {
            dn,
            display_name,
            children: None,
            has_children_hint: true,
        }
    }

    /// Whether this node has been loaded (children fetched).
    pub fn is_loaded(&self) -> bool {
        self.children.is_some()
    }

    /// Whether this node is expanded (loaded and has children).
    pub fn is_expanded(&self) -> bool {
        self.children.as_ref().is_some_and(|c| !c.is_empty())
    }

    /// Set the children of this node.
    pub fn set_children(&mut self, children: Vec<TreeNode>) {
        self.has_children_hint = !children.is_empty();
        self.children = Some(children);
    }

    /// Collapse this node (remove children from memory).
    pub fn collapse(&mut self) {
        self.children = None;
    }
}

/// The full directory tree, lazily loaded.
#[derive(Debug)]
pub struct DirectoryTree {
    pub root_dn: String,
    pub root: TreeNode,
}

impl DirectoryTree {
    pub fn new(root_dn: String) -> Self {
        let root = TreeNode::new(root_dn.clone());
        Self { root_dn, root }
    }

    /// Find a mutable reference to a node by DN.
    pub fn find_node_mut(&mut self, target_dn: &str) -> Option<&mut TreeNode> {
        Self::find_in_node(&mut self.root, target_dn)
    }

    fn find_in_node<'a>(node: &'a mut TreeNode, target_dn: &str) -> Option<&'a mut TreeNode> {
        if node.dn.eq_ignore_ascii_case(target_dn) {
            return Some(node);
        }
        if let Some(ref mut children) = node.children {
            for child in children.iter_mut() {
                if let Some(found) = Self::find_in_node(child, target_dn) {
                    return Some(found);
                }
            }
        }
        None
    }

    /// Insert children for a specific node DN.
    pub fn insert_children(&mut self, parent_dn: &str, children: Vec<TreeNode>) {
        if let Some(node) = self.find_node_mut(parent_dn) {
            node.set_children(children);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_node_new() {
        let node = TreeNode::new("cn=Admin,dc=example,dc=com".to_string());
        assert_eq!(node.dn, "cn=Admin,dc=example,dc=com");
        assert_eq!(node.display_name, "Admin");
        assert!(node.has_children_hint);
        assert!(!node.is_loaded());
        assert!(!node.is_expanded());
    }

    #[test]
    fn test_tree_node_set_children() {
        let mut node = TreeNode::new("dc=example,dc=com".to_string());
        assert!(!node.is_loaded());

        node.set_children(vec![
            TreeNode::new("ou=Users,dc=example,dc=com".to_string()),
            TreeNode::new("ou=Groups,dc=example,dc=com".to_string()),
        ]);

        assert!(node.is_loaded());
        assert!(node.is_expanded());
        assert_eq!(node.children.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_tree_node_set_empty_children() {
        let mut node = TreeNode::new("ou=Empty,dc=example,dc=com".to_string());
        node.set_children(vec![]);

        assert!(node.is_loaded());
        assert!(!node.is_expanded()); // loaded but no children
        assert!(!node.has_children_hint);
    }

    #[test]
    fn test_tree_node_collapse() {
        let mut node = TreeNode::new("dc=example,dc=com".to_string());
        node.set_children(vec![TreeNode::new(
            "ou=Users,dc=example,dc=com".to_string(),
        )]);
        assert!(node.is_loaded());

        node.collapse();
        assert!(!node.is_loaded());
        assert!(!node.is_expanded());
    }

    #[test]
    fn test_directory_tree_find_root() {
        let mut tree = DirectoryTree::new("dc=example,dc=com".to_string());
        let found = tree.find_node_mut("dc=example,dc=com");
        assert!(found.is_some());
        assert_eq!(found.unwrap().dn, "dc=example,dc=com");
    }

    #[test]
    fn test_directory_tree_find_child() {
        let mut tree = DirectoryTree::new("dc=example,dc=com".to_string());
        tree.insert_children(
            "dc=example,dc=com",
            vec![
                TreeNode::new("ou=Users,dc=example,dc=com".to_string()),
                TreeNode::new("ou=Groups,dc=example,dc=com".to_string()),
            ],
        );

        let found = tree.find_node_mut("ou=Groups,dc=example,dc=com");
        assert!(found.is_some());
        assert_eq!(found.unwrap().display_name, "Groups");
    }

    #[test]
    fn test_directory_tree_find_not_found() {
        let mut tree = DirectoryTree::new("dc=example,dc=com".to_string());
        assert!(tree.find_node_mut("cn=missing,dc=example,dc=com").is_none());
    }

    #[test]
    fn test_directory_tree_case_insensitive_find() {
        let mut tree = DirectoryTree::new("DC=example,DC=com".to_string());
        let found = tree.find_node_mut("dc=example,dc=com");
        assert!(found.is_some());
    }

    #[test]
    fn test_directory_tree_insert_nested_children() {
        let mut tree = DirectoryTree::new("dc=example,dc=com".to_string());

        // Add first level
        tree.insert_children(
            "dc=example,dc=com",
            vec![TreeNode::new("ou=Users,dc=example,dc=com".to_string())],
        );

        // Add second level
        tree.insert_children(
            "ou=Users,dc=example,dc=com",
            vec![TreeNode::new(
                "cn=Alice,ou=Users,dc=example,dc=com".to_string(),
            )],
        );

        // Find nested child
        let found = tree.find_node_mut("cn=Alice,ou=Users,dc=example,dc=com");
        assert!(found.is_some());
        assert_eq!(found.unwrap().display_name, "Alice");
    }
}
