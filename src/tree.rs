use std::collections::BTreeMap;
use std::process::Command;
use std::path::PathBuf;
use gix;
use anyhow::Result;

pub struct GitTree;

// Recursive type for our tree structure
enum TreeNode {
    File,
    Dir(BTreeMap<String, TreeNode>),
}

impl GitTree {
    pub fn get_git_root() -> Result<PathBuf> {
        // Discover repository from current directory
        let repo = gix::discover(".")?;
        
        // Get the working directory path and canonicalize it to get absolute path
        let root_path = repo.work_dir()
            .ok_or_else(|| anyhow::anyhow!("Repository has no working directory (might be bare)"))?
            .canonicalize()?;
        
        Ok(root_path)
    }

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

        let mut result = String::from(".
");
        Self::build_tree_string(&tree, "", &mut result);
        Ok(result)
    }

    fn build_tree_string(
        tree: &BTreeMap<String, TreeNode>,
        prefix: &str,
        result: &mut String,
    ) {
        for (i, (name, node)) in tree.iter().enumerate() {
            let is_last_entry = i == tree.len() - 1;
            let connector = if is_last_entry { "└── " } else { "├── " };
            let next_prefix = if is_last_entry { "    " } else { "│   " };

            result.push_str(&format!("{}{}{}
", prefix, connector, name));

            if let TreeNode::Dir(subtree) = node {
                Self::build_tree_string(subtree, &format!("{}{}", prefix, next_prefix), result);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_get_git_root() -> Result<()> {
        // Save current directory
        let original_dir = env::current_dir()?;
        
        // Create a temporary directory structure
        let temp_dir = tempfile::tempdir()?;
        
        // Initialize a git repo in the temp directory
        let _repo = gix::init(temp_dir.path())?;
        
        // Change to a subdirectory to test discovery
        let subdir = temp_dir.path().join("src").join("nested");
        std::fs::create_dir_all(&subdir)?;
        env::set_current_dir(&subdir)?;
        
        // Test finding the root
        let root = GitTree::get_git_root()?;
        
        assert_eq!(root, temp_dir.path().canonicalize()?);
        
        // Restore original directory
        env::set_current_dir(original_dir)?;
        Ok(())
    }
}

