> Persistent key-value store backed by GIT.

This package provides a key-value store that uses GIT for the permanent storage.

## Example

```rs
use gitmap::Repo;

let path = Path::new("/storage/path");
let map = Repo::init(path);
map.insert_key("key1", "value1".to_bytes());
map.insert_key("key2", "value2".to_bytes());
map.commit("First commit");
```

## Methods

```rs
// Constructors
Repo::init(path);
Repo::open(path);
// Overview
repo.path();
repo.is_empty();
// Branches
repo.branches();
repo.branch();
repo.switch_branch(name);
repo.remove_branch();
repo.has_branch(name);
// Keys
repo.keys();
repo.value(key);
repo.insert_key(name, value);
repo.remove_key(name);
repo.reset_key(name);
repo.has_key(key);
repo.key_changed(key);
repo.len();
// Operations
repo.changed();
repo.remove();
repo.reset();
repo.commit(message);
repo.rollback();
```

## To-do

* Add `rollback()`
