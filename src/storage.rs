use anyhow::Result;
use rocksdb::{DB, Options, IteratorMode};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseInfo {
    pub key_count: usize,
    pub size_bytes: u64,
}

pub trait Storage: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn set(&self, key: &str, value: &str) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
    fn list_keys(&self) -> Result<Vec<String>>;
    fn info(&self) -> Result<DatabaseInfo>;
    fn exists(&self, key: &str) -> Result<bool>;
}

pub struct RocksDBStorage {
    db: Arc<DB>,
}

impl RocksDBStorage {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
        
        let db = DB::open(&opts, path)?;
        
        Ok(Self {
            db: Arc::new(db),
        })
    }
}

impl Storage for RocksDBStorage {
    fn get(&self, key: &str) -> Result<Option<String>> {
        match self.db.get(key.as_bytes())? {
            Some(value) => Ok(Some(String::from_utf8(value)?)),
            None => Ok(None),
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<()> {
        self.db.put(key.as_bytes(), value.as_bytes())?;
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<()> {
        self.db.delete(key.as_bytes())?;
        Ok(())
    }

    fn list_keys(&self) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        let iter = self.db.iterator(IteratorMode::Start);
        
        for item in iter {
            let (key, _) = item?;
            if let Ok(key_str) = String::from_utf8(key.to_vec()) {
                keys.push(key_str);
            }
        }
        
        Ok(keys)
    }

    fn info(&self) -> Result<DatabaseInfo> {
        let mut key_count = 0;
        let mut size_bytes = 0;
        
        let iter = self.db.iterator(IteratorMode::Start);
        for item in iter {
            let (key, value) = item?;
            key_count += 1;
            size_bytes += key.len() + value.len();
        }
        
        Ok(DatabaseInfo {
            key_count,
            size_bytes: size_bytes as u64,
        })
    }

    fn exists(&self, key: &str) -> Result<bool> {
        Ok(self.db.get(key.as_bytes())?.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_basic_operations() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let storage = RocksDBStorage::new(temp_dir.path())?;
        
        // Test set and get
        storage.set("key1", "value1")?;
        assert_eq!(storage.get("key1")?, Some("value1".to_string()));
        
        // Test non-existent key
        assert_eq!(storage.get("nonexistent")?, None);
        
        // Test exists
        assert!(storage.exists("key1")?);
        assert!(!storage.exists("nonexistent")?);
        
        // Test delete
        storage.delete("key1")?;
        assert_eq!(storage.get("key1")?, None);
        assert!(!storage.exists("key1")?);
        
        // Test list keys
        storage.set("key2", "value2")?;
        storage.set("key3", "value3")?;
        let keys = storage.list_keys()?;
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"key2".to_string()));
        assert!(keys.contains(&"key3".to_string()));
        
        Ok(())
    }
}
