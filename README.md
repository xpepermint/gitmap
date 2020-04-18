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

## To-do

* Add `rollback()`
