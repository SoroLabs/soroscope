// core/src/engine/noop_cache.rs
use super::traits::{StateCache, CacheError};
use async_trait::async_trait;

#[derive(Debug, Clone, Default)]
pub struct NoOpCache;

impl NoOpCache {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StateCache for NoOpCache {
    async fn get(&self, _key: &str) -> Option<Vec<u8>> {
        None
    }
    
    async fn set(&self, _key: &str, _value: Vec<u8>) -> Result<(), CacheError> {
        Ok(())
    }
    
    async fn invalidate(&self, _key: &str) -> Result<(), CacheError> {
        Ok(())
    }
}
