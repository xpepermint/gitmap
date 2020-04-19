use std::path::{Path};
use git2::{Repository, BranchType, Oid, DiffOptions};

pub use git2::Error;

/// Structure properties.
pub struct Repo {
    /// Git2 repository reference.
    repo: Repository,
    /// Temporial tree id.
    tree_id: Option<Oid>,
}

/// Repo functions.
impl Repo {

    /// Creates a new `--bare` repository in the specified folder.
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        Ok(Self::new(Repository::init_bare(path)?))
    }

    /// Opens an existing repository.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        Ok(Self::new(Repository::open_bare(path)?))
    }

    /// Returns a new repo object.
    fn new(repo: Repository) -> Self {
        Self {
            repo,
            tree_id: None,
        }
    }

    /// Returns repository path.
    pub fn path(&self) -> &Path {
        self.repo.path()
    }

    /// Returns the number of all keys.
    pub fn len(&self) -> usize {
        self.keys().len()
    }

    /// Returns true if repository has no commits.
    pub fn has_commits(&self) -> bool {
        match self.repo.is_empty() {
            Ok(v) => !v,
            Err(_) => false,
        }
    }

    /// Returns true if at least one branch exists.
    pub fn has_branches(&self) -> bool {
        self.branches().len() > 0
    }

    /// Returns true if at least one key exists.
    pub fn has_keys(&self) -> bool {
        self.keys().len() > 0
    }

    /// Returns true if the provided branch exists.
    pub fn has_branch(&self, name: &str) -> bool {
        self.branches().contains(&name.to_string())
    }

    /// Returns true if the key exists.
    pub fn has_key(&self, name: &str) -> bool {
        let tree = match self.current_tree_id() {
            Ok(id) => match self.repo.find_tree(id) {
                Ok(tree) => tree,
                Err(_) => return false,
            },
            Err(_) => return false,
        };
        let name = tree.get_name(name);
        name.is_some()
    }
    
    /// Returns working branch name.
    pub fn branches(&self) -> Vec<String> {
        let mut names = Vec::new();

        let branches = match self.repo.branches(Some(BranchType::Local)) {
            Err(_) => return names,
            Ok(iter) => iter,
        };

        for item in branches {
            names.push(
                match item {
                    Ok(i) => match i.0.name() {
                        Ok(n) => match n {
                            Some(n) => n.to_string(),
                            None => break,
                        },
                        Err(_) => break,
                    },
                    Err(_) => break,
                },
            );
        }
        names
    }

    /// List all available keys.
    pub fn keys(&self) -> Vec<String> {
        let mut paths: Vec<String> = Vec::new();

        let tree = match self.current_tree_id() {
            Ok(id) => match self.repo.find_tree(id) {
                Ok(tree) => tree,
                Err(_) => return paths,
            },
            Err(_) => return paths,
        };
        let mut opts = DiffOptions::new();
            opts.include_unmodified(true);
        let diff = match self.repo.diff_tree_to_tree(Some(&tree), None, Some(&mut opts)) {
            Ok(diff) => diff,
            Err(_) => return paths,
        };
        
        for item in diff.deltas() {
            paths.push(
                match item.old_file().path() {
                    Some(path) => match path.to_str() {
                        Some(path) => path.to_string(),
                        None => continue,
                    },
                    None => continue,
                },
            );
        }
        paths
    }

    /// Returns working branch name.
    pub fn branch(&self) -> Option<String> {
        match self.repo.head() {
            Ok(head) => match head.name() {
                Some(name) => match name.split("refs/heads/").last() {
                    Some(name) => Some(name.to_string()),
                    None => None,
                },
                None => None,
            },
            Err(_) => None,
        }
    }

    /// Retrieves key content.
    pub fn key(&self, name: &str) -> Option<Vec<u8>> {
        let tree = match self.current_tree_id() {
            Ok(id) => match self.repo.find_tree(id) {
                Ok(tree) => tree,
                Err(_) => return None,
            },
            Err(_) => return None,
        };
        let content = match tree.get_name(name) {
            Some(entry) => match entry.to_object(&self.repo) {
                Ok(blob) => match blob.as_blob() {
                    Some(data) => data.content().to_vec(),
                    None => return None,
                },
                Err(_) => return None,
            },
            None => return None,
        };
        Some(content)
    }
    
    /// Ensures new working branch. There must be at least one commit in the
    /// repository for this method to work other wise the error is thrown.
    pub fn switch_branch(&mut self, name: &str) -> Result<(), Error> {
        if !self.has_branch(name) {
            let commit = self.repo.find_commit(self.last_commit_id()?)?;
            self.repo.branch(name, &commit, false)?;
        }
        self.repo.set_head(
            format!("refs/heads/{}", name).as_str(),
        )?;
        Ok(())
    }

    /// Removes working branch. Note that the current branch can not be removed
    /// and you have to first switch to a new branch.
    pub fn remove_branch(&mut self, name: &str) -> Result<(), Error> {
        self.repo.find_branch(&name, BranchType::Local)?.delete()
    }

    /// Stages key for commit.
    pub fn insert_key(&mut self, name: &str, value: &[u8]) -> Result<(), Error> {
        let tree = self.repo.find_tree(self.current_tree_id()?)?;
        let file_oid = self.repo.blob(value)?;
        let mut builder = self.repo.treebuilder(Some(&tree))?;
        builder.insert(name, file_oid, 0o100644)?;
        self.tree_id = Some(builder.write()?);
        Ok(())
    }

    /// Reset all keys.
    pub fn reset(&mut self) -> Result<(), Error> {
        self.tree_id = None;
        Ok(())
    }

    /// Remove all keys.
    pub fn remove(&mut self) -> Result<(), Error> {
        let tree = self.repo.find_tree(self.current_tree_id()?)?;
        let mut builder = self.repo.treebuilder(Some(&tree))?;
        for key in self.keys() {
            builder.remove(key)?;
        }
        self.tree_id = Some(builder.write()?);
        Ok(())
    }

    /// Returns true if any key has been changed.
    pub fn changed(&self) -> bool {
        if !self.has_commits() {
            return self.tree_id.is_some() && self.len() > 0;
        }
        let mut opts = DiffOptions::new();
        let old_tree = match self.last_tree_id() {
            Ok(id) => match self.repo.find_tree(id) {
                Ok(tree) => tree,
                Err(_) => return false,
            },
            Err(_) => return false,
        };
        let new_tree = match self.current_tree_id() {
            Ok(id) => match self.repo.find_tree(id) {
                Ok(tree) => tree,
                Err(_) => return false,
            },
            Err(_) => return false,
        };
        let diff = match self.repo.diff_tree_to_tree(Some(&old_tree), Some(&new_tree), Some(&mut opts)) {
            Ok(diff) => diff,
            Err(_) => return false,
        };
        diff.deltas().len() > 0
    }

    /// Commits data.
    pub fn commit(&self, message: &str) -> Result<(), Error> {
        let tree_id = self.current_tree_id()?;
        let tree = self.repo.find_tree(tree_id)?;
        let sig = self.repo.signature()?;
        if !self.has_commits() {
            self.repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[])?;
        } else {
            let commit = self.repo.find_commit(self.last_commit_id()?)?;
            self.repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&commit])?;
        }
        Ok(())
    }

    /// Stages key for removal.
    pub fn reset_key(&mut self, name: &str) -> Result<(), Error> {
        let tree = self.repo.find_tree(self.current_tree_id()?)?;
        let mut builder = self.repo.treebuilder(Some(&tree))?;
        if self.has_key(name) {
            builder.remove(name)?;
        }
        if self.has_commits() {
            let tree = self.repo.find_tree(self.last_tree_id()?)?;
            if tree.get_name(name) != None {
                let entry = tree.get_path(Path::new(name))?;
                let blob = entry.to_object(&self.repo)?;
                let blob = blob.as_blob().unwrap();
                let oid = self.repo.blob(&blob.content())?;
                builder.insert(name, oid, 0o100644)?;
            }
        }
        self.tree_id = Some(builder.write()?);
        Ok(())
    }
    
    /// Stages key for removal.
    pub fn remove_key(&mut self, name: &str) -> Result<(), Error> {
        if self.has_key(name) {
            let tree = self.repo.find_tree(self.current_tree_id()?)?;
            let mut builder = self.repo.treebuilder(Some(&tree))?;
            builder.remove(name)?;
            self.tree_id = Some(builder.write()?);
        }
        Ok(())
    }
    
    /// Returns true if the key content has been changed.
    pub fn key_changed(&self, name: &str) -> bool {
        if !self.has_commits() {
            return self.has_key(name);
        }
        let mut opts = DiffOptions::new();
        let old_tree = match self.last_tree_id() {
            Ok(id) => match self.repo.find_tree(id) {
                Ok(tree) => tree,
                Err(_) => return false,
            },
            Err(_) => return false,
        };
        let new_tree = match self.current_tree_id() {
            Ok(id) => match self.repo.find_tree(id) {
                Ok(tree) => tree,
                Err(_) => return false,
            },
            Err(_) => return false,
        };
        let diff = match self.repo.diff_tree_to_tree(Some(&old_tree), Some(&new_tree), Some(&mut opts)) {
            Ok(diff) => diff,
            Err(_) => return false,
        };
        for delta in diff.deltas() {
            match delta.new_file().path() {
                Some(path) => match path.to_str() {
                    Some(n) => match n == name {
                        true => return true,
                        false => continue,
                    },
                    None => continue,
                },
                None => continue,
            };
        }
        false
   }
    
    /// Roll back one commit.
    // pub fn rollback(&self) -> Result<(), Error> {
    //     // Hints (I think):
    //     // Normal repo: git reset --hard <commit-oid>
    //     // Bare repo: git update-ref refs/heads/master <old-tree-oid>
    //     Ok(())
    // }

    /// Creates an empty tree and returns its ID.
    fn empty_tree_id(&self) -> Result<Oid, Error> {
        Ok(self.repo.treebuilder(None)?.write()?)
    }

    /// Current working tree ID.
    fn current_tree_id(&self) -> Result<Oid, Error> {
        if self.tree_id.is_some() {
            Ok(self.tree_id.unwrap())
        } else if !self.has_commits() {
            self.empty_tree_id()
        } else {
            self.last_tree_id()
        }
    }
    
    /// Last commited tree ID.
    fn last_tree_id(&self) -> Result<Oid, Error> {
        Ok(self.repo.find_commit(self.last_commit_id()?)?.tree_id())
    }

    /// Last commit ID. 
    fn last_commit_id(&self) -> Result<Oid, Error> {
        Ok(self.repo.revparse_single("HEAD")?.id())
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;
    use super::*;

    #[test]
    fn initializes_repository() {
        let path = TempDir::new().unwrap().path().to_owned();
        let repo = Repo::init(&path).unwrap();
        assert_eq!(repo.path().join("config").exists(), true);
    }

    #[test]
    fn opens_repository() {
        let path = TempDir::new().unwrap().path().to_owned();
        Repo::init(&path).unwrap();
        let repo = Repo::open(&path).unwrap();
        assert_eq!(repo.path().join("config").exists(), true);
    }

    #[test]
    fn checks_commits_existance() {
        let path = TempDir::new().unwrap().path().to_owned();
        let repo = Repo::init(&path).unwrap();
        assert_eq!(repo.has_commits(), false);
        repo.commit("").unwrap(); // initial commit
        assert_eq!(repo.has_commits(), true);
    }

    #[test]
    fn checks_branches_existance() {
        let path = TempDir::new().unwrap().path().to_owned();
        let repo = Repo::init(&path).unwrap();
        assert_eq!(repo.has_branches(), false);
        repo.commit("").unwrap(); // initial commit
        assert_eq!(repo.has_branches(), true);
    }

    #[test]
    fn checks_keys_existance() {
        let path = TempDir::new().unwrap().path().to_owned();
        let mut repo = Repo::init(&path).unwrap();
        assert_eq!(repo.has_keys(), false);
        repo.insert_key("bar", "".as_bytes()).unwrap();
        repo.commit("").unwrap(); // initial commit
        assert_eq!(repo.has_keys(), true);
    }

    #[test]
    fn checks_branch_existance() {
        let path = TempDir::new().unwrap().path().to_owned();
        let mut repo = Repo::init(&path).unwrap();
        assert_eq!(repo.has_branch("master"), false); // empty repository
        repo.commit("").unwrap(); // initial commit
        assert_eq!(repo.has_branch("master"), true);
        repo.switch_branch("foo").unwrap();
        repo.switch_branch("bar").unwrap();
        assert_eq!(repo.has_branch("foo"), true);
    }

    #[test]
    fn checks_key_existance() {
        let path = TempDir::new().unwrap().path().to_owned();
        let mut repo = Repo::init(&path).unwrap();
        assert_eq!(repo.has_key("bar"), false);
        repo.insert_key("foo", "111".as_bytes()).unwrap();
        repo.insert_key("bar", "222".as_bytes()).unwrap();
        repo.insert_key("baz", "333".as_bytes()).unwrap();
        repo.commit("").unwrap();
        assert_eq!(repo.has_key("bar"), true);
    }

    #[test]
    fn provides_branches() {
        let path = TempDir::new().unwrap().path().to_owned();
        let repo = Repo::init(&path).unwrap();
        assert_eq!(repo.branches().len(), 0);
        repo.commit("").unwrap(); // initial commit
        assert_eq!(repo.branches(), ["master"]);
    }

    #[test]
    fn provides_keys() {
        let path = TempDir::new().unwrap().path().to_owned();
        let mut repo = Repo::init(&path).unwrap();
        assert_eq!(repo.keys().len(), 0);
        repo.insert_key("foo", "".as_bytes()).unwrap();
        repo.insert_key("bar", "".as_bytes()).unwrap();
        repo.commit("").unwrap();
        assert_eq!(repo.keys(), ["bar", "foo"]);
    }

    #[test]
    fn provides_current_branch() {
        let path = TempDir::new().unwrap().path().to_owned();
        let repo = Repo::init(&path).unwrap();
        assert_eq!(repo.branch().is_none(), true);
        repo.commit("").unwrap(); // initial commit
        assert_eq!(repo.branch().unwrap(), "master");
    }
    
    #[test]
    fn provides_key_value() {
        let path = TempDir::new().unwrap().path().to_owned();
        let mut repo = Repo::init(&path).unwrap();
        assert_eq!(repo.key("bar").is_none(), true);
        repo.insert_key("foo", "111".as_bytes()).unwrap();
        repo.insert_key("bar", "222".as_bytes()).unwrap();
        repo.insert_key("baz", "333".as_bytes()).unwrap();
        repo.commit("").unwrap();
        assert_eq!(String::from_utf8(repo.key("bar").unwrap()).unwrap(), "222");
    }

    #[test]
    fn switches_branch() {
        let path = TempDir::new().unwrap().path().to_owned();
        let mut repo = Repo::init(&path).unwrap();
        assert_eq!(repo.switch_branch("foo").is_err(), true);
        repo.commit("").unwrap(); // initial commit
        assert_eq!(repo.switch_branch("foo").is_ok(), true);
        assert_eq!(repo.branch().unwrap(), "foo");
    }

    #[test]
    fn removes_branch() {
        let path = TempDir::new().unwrap().path().to_owned();
        let mut repo = Repo::init(&path).unwrap();
        assert_eq!(repo.remove_branch("master").is_err(), true); // no branches
        repo.commit("").unwrap(); // initial commit
        assert_eq!(repo.remove_branch("master").is_err(), true); // can't remove working branch
        repo.switch_branch("foo").unwrap();
        assert_eq!(repo.branches(), ["foo", "master"]);
        assert_eq!(repo.remove_branch("foo").is_err(), true); // can't remove working branch
        repo.switch_branch("master").unwrap();
        assert_eq!(repo.remove_branch("foo").is_ok(), true);
        assert_eq!(repo.branches(), ["master"]);
    }

    #[test]
    fn performs_operations() {
        let path = TempDir::new().unwrap().path().to_owned();
        let mut repo = Repo::init(&path).unwrap();
        repo.insert_key("foo", "1".as_bytes()).unwrap();
        repo.commit("").unwrap();
        assert_eq!(repo.keys(), ["foo"]);
        repo.insert_key("foo", "11".as_bytes()).unwrap();
        repo.insert_key("bar", "2".as_bytes()).unwrap();
        repo.commit("").unwrap();
        assert_eq!(repo.keys(), ["bar", "foo"]);
        repo.remove_key("foo").unwrap();
        repo.insert_key("bar", "22".as_bytes()).unwrap();
        repo.reset_key("bar").unwrap();
        repo.commit("").unwrap();
        assert_eq!(repo.keys(), ["bar"]);
        assert_eq!(String::from_utf8(repo.key("bar").unwrap()).unwrap(), "2");
        repo.remove().unwrap();
        repo.commit("").unwrap();
        assert_eq!(repo.keys().len(), 0);
    }

    #[test]
    fn checks_changes() {
        let path = TempDir::new().unwrap().path().to_owned();
        let mut repo = Repo::init(&path).unwrap();
        assert_eq!(repo.changed(), false);
        repo.insert_key("foo", "".as_bytes()).unwrap();
        assert_eq!(repo.changed(), true);
        repo.reset().unwrap();
        assert_eq!(repo.changed(), false);
    }

    #[test]
    fn checks_key_changes() {
        let path = TempDir::new().unwrap().path().to_owned();
        let mut repo = Repo::init(&path).unwrap();
        assert_eq!(repo.key_changed("foo"), false);
        repo.insert_key("foo", "".as_bytes()).unwrap();
        repo.insert_key("bar", "".as_bytes()).unwrap();
        assert_eq!(repo.key_changed("foo"), true);
        assert_eq!(repo.key_changed("bar"), true);
        repo.reset_key("foo").unwrap();
        assert_eq!(repo.key_changed("foo"), false);
        assert_eq!(repo.key_changed("bar"), true);
    }
}
