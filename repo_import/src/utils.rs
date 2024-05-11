use lazy_static::lazy_static;
use model::crate_info::{Program, UProgram};
use std::collections::HashMap;
use std::sync::Mutex;

lazy_static! {
    pub static ref NAMESPACE_HASHMAP: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
}

pub fn insert_namespace_by_repo_path(key: String, value: String) {
    let mut map = NAMESPACE_HASHMAP.lock().unwrap();
    map.insert(key, value);
}

pub fn get_namespace_by_repo_path(key: &str) -> Option<String> {
    let map = NAMESPACE_HASHMAP.lock().unwrap();
    map.get(key).cloned()
}

lazy_static! {
    pub static ref PROGRAM_HASHMAP: Mutex<HashMap<String, (Program, UProgram)>> =
        Mutex::new(HashMap::new());
}

pub fn insert_program_by_name(key: String, value: (Program, UProgram)) {
    let mut map = PROGRAM_HASHMAP.lock().unwrap();
    map.insert(key, value);
}

pub fn get_program_by_name(key: &str) -> Option<(Program, UProgram)> {
    let map = PROGRAM_HASHMAP.lock().unwrap();
    map.get(key).cloned()
}
