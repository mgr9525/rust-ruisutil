use std::{
    collections::{hash_map::Iter, HashMap},
    ops::Index,
    path::{Path, PathBuf},
};

pub struct KVConfig {
    mp: HashMap<String, String>,
}

impl KVConfig {
    pub fn new() -> Self {
        Self { mp: HashMap::new() }
    }
    pub fn from_file<P: Into<PathBuf>>(pth: P) -> Self {
        let mut kvc = Self::new();
        match std::fs::read(pth.into()) {
            Err(e) => println!("kv from file err:{}", e),
            Ok(v) => kvc.parses(&v),
        }
        kvc
    }
    pub fn from_bytes(bts: &[u8]) -> Self {
        let mut kvc = Self::new();
        kvc.parses(bts);
        kvc
    }

    fn parses(&mut self, bts: &[u8]) {
        let ln = bts.len();
        let mut start = 0;
        let mut idx_dh = 0;
        for i in 0..ln {
            if bts[i] == b'=' {
                idx_dh = i;
            }
            if bts[i] == b'\n' || i == ln - 1 {
                if idx_dh > 0 && idx_dh < i {
                    if let Ok(k) = std::str::from_utf8(&bts[start..idx_dh]) {
                        if idx_dh + 1 >= i {
                            self.mp.insert(k.trim().to_string(), String::new());
                        } else if let Ok(v) = std::str::from_utf8(&bts[idx_dh + 1..i]) {
                            self.mp.insert(k.trim().to_string(), v.trim().to_string());
                        }
                    }
                }
                start = i + 1;
                idx_dh = 0;
            }
        }
    }

    pub fn get<T: Into<String>>(&self, key: T) -> Option<&String> {
        self.mp.get(&key.into())
    }
    pub fn geti<T: Into<String>>(&self, key: T) -> Option<i64> {
        match self.mp.get(&key.into()) {
            None => None,
            Some(vs) => match vs.parse::<i64>() {
                Ok(v) => Some(v),
                Err(_) => None,
            },
        }
    }
    pub fn set<T: Into<String>>(&mut self, key: T, val: T) {
        self.mp.insert(key.into(), val.into());
    }
    pub fn iter(&self) -> Iter<String, String> {
        self.mp.iter()
    }
    pub fn to_string(&self) -> String {
        let mut cont = String::new();
        for (k, v) in &self.mp {
            cont.push_str(format!("{}={}\n", k, v).as_str());
        }
        cont
    }
}
