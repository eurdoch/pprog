use std::collections::BTreeMap;
use std::process::Command;

pub struct GitTree;

// Recursive type for our tree structure
enum TreeNode {
    File,
    Dir(BTreeMap<String, TreeNode>),
}

impl GitTree {
    pub fn get_tree() -> Result<String, std::io::Error> {
        let output = Command::new("git")
            .arg("ls-files")
            .output()?;

        if !output.status.success() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }

        let files: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(String::from)
            .collect();

        let mut tree = BTreeMap::new();
        for path in files {
            let parts: Vec<&str> = path.split('/').collect();
            let file_name = parts.last().unwrap();
            let dir_parts = &parts[..parts.len() - 1];

            let mut current = &mut tree;
            for &part in dir_parts {
                current = current
                    .entry(part.to_string())
                    .or_insert(TreeNode::Dir(BTreeMap::new()))
                    .as_dir_mut()
                    .unwrap();
            }
            current.insert(file_name.to_string(), TreeNode::File);
        }

        let mut result = String::from(".\n");
        Self::build_tree_string(&tree, "", true, &mut result);
        Ok(result)
    }

    fn build_tree_string(
        tree: &BTreeMap<String, TreeNode>,
        prefix: &str,
        is_last: bool,
        result: &mut String,
    ) {
        for (i, (name, node)) in tree.iter().enumerate() {
            let is_last_entry = i == tree.len() - 1;
            let connector = if is_last_entry { "└── " } else { "├── " };
            let next_prefix = if is_last_entry { "    " } else { "│   " };

            result.push_str(&format!("{}{}{}\n", prefix, connector, name));

            if let TreeNode::Dir(subtree) = node {
                Self::build_tree_string(subtree, &format!("{}{}", prefix, next_prefix), is_last_entry, result);
            }
        }
    }
}

impl TreeNode {
    fn as_dir_mut(&mut self) -> Option<&mut BTreeMap<String, TreeNode>> {
        match self {
            TreeNode::Dir(map) => Some(map),
            _ => None,
        }
    }
}
