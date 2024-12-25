use std::collections::BTreeMap;
use std::process::Command;

pub struct GitTree;

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
            let mut current = &mut tree;
            for part in path.split('/') {
                current = current.entry(part.to_string()).or_insert_with(BTreeMap::new);
            }
        }

        let mut result = String::from(".\n");
        Self::build_tree_string(&tree, "", true, &mut result);
        Ok(result)
    }

    fn build_tree_string(
        tree: &BTreeMap<String, BTreeMap<String, String>>, 
        prefix: &str, 
        is_last: bool,
        result: &mut String,
    ) {
        for (i, (name, subtree)) in tree.iter().enumerate() {
            let is_last_entry = i == tree.len() - 1;
            let connector = if is_last_entry { "└── " } else { "├── " };
            let next_prefix = if is_last_entry { "    " } else { "│   " };

            result.push_str(&format!("{}{}{}\n", prefix, connector, name));

            if !subtree.is_empty() {
                Self::build_tree_string(subtree, &format!("{}{}", prefix, next_prefix), is_last_entry, result);
            }
        }
    }
}
