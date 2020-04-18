use gitmap::{Repo};
use tempfile::{TempDir};

#[test]
fn initializes_repository() {
    let path = TempDir::new().unwrap().path().to_owned();
    let map = Repo::init(&path).unwrap();
    assert_eq!(map.path().join("config").exists(), true);
}
