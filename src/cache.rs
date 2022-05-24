use anyhow::anyhow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ya_client_model::NodeId;

pub struct Cache {
    path: PathBuf,
    nodes: HashMap<NodeId, String>,
}

impl Cache {
    pub async fn new(path: &Path) -> anyhow::Result<Cache> {
        let mut cache = Cache {
            path: path.to_path_buf(),
            nodes: HashMap::new(),
        };

        if path.exists() {
            cache
                .load(path)
                .await
                .map_err(|e| anyhow!("Failed to load cache from: {}. {}", path.display(), e))?;
        }

        Ok(cache)
    }

    async fn load(&mut self, file: &Path) -> anyhow::Result<()> {
        let content = tokio::fs::read(file).await?;
        self.nodes = serde_json::from_slice::<HashMap<NodeId, String>>(&content)?;
        Ok(())
    }

    pub async fn update_cache(&mut self, nodes_info: HashMap<NodeId, String>) {
        let mut updated: bool = false;

        for (id, name) in nodes_info {
            if self.nodes.insert(id, name).is_some() {
                updated = true;
            }
        }

        if updated {
            self.save()
                .await
                .map_err(|e| log::warn!("Failed to save updated nodes information to cache. {}", e))
                .ok();
        }
    }

    async fn save(&self) -> anyhow::Result<()> {
        tokio::fs::create_dir_all(&self.path).await?;

        let content = serde_json::to_string(&self.nodes)?;
        tokio::fs::write(&self.path, content.as_bytes()).await?;
        Ok(())
    }

    pub fn node_name(&self, id: NodeId) -> Option<String> {
        self.nodes.get(&id).cloned()
    }
}
