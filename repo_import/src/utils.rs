use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::Mutex;

lazy_static! {
    static ref GLOBAL_HASHMAP: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
}

pub fn insert_info(key: String, value: String) {
    let mut map = GLOBAL_HASHMAP.lock().unwrap();
    map.insert(key, value);
}

pub fn _get_info(key: &str) -> Option<String> {
    let map = GLOBAL_HASHMAP.lock().unwrap();
    map.get(key).cloned()
}
