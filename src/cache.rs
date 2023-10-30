use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use poise::serenity_prelude::GuildId;

use crate::{config::Config, score::Score};

#[derive(Debug, Default)]
pub struct DataCache {
    configs: HashMap<GuildId, Config>,
    scores: HashMap<GuildId, Cache<Arc<[Score]>>>,
}

impl DataCache {
    pub fn get_config(&self, guild_id: GuildId) -> Option<&Config> {
        self.configs.get(&guild_id)
    }

    pub fn get_mut_config(&mut self, guild_id: GuildId) -> Option<&mut Config> {
        self.configs.get_mut(&guild_id)
    }

    pub fn insert_config(&mut self, guild_id: GuildId, config: Config) -> Option<Config> {
        self.configs.insert(guild_id, config)
    }

    pub fn set_scores(&mut self, guild_id: GuildId, scores: Arc<[Score]>) {
        let scores = Cache::new(scores, 3600);
        self.scores.insert(guild_id, scores);
    }

    pub fn get_scores(&self, guild_id: GuildId) -> Option<&Cache<Arc<[Score]>>> {
        self.scores.get(&guild_id)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Cache<T> {
    inner: T,
    ttl: Duration,
    timestamp: Instant,
}

impl<T> Cache<T> {
    pub fn new(inner: T, ttl: u64) -> Cache<T> {
        let timestamp = Instant::now();
        let ttl = Duration::from_secs(ttl);

        Cache {
            inner,
            ttl,
            timestamp,
        }
    }

    pub fn get(&self) -> Option<&T> {
        if self.is_expired() {
            None
        } else {
            Some(&self.inner)
        }
    }

    fn is_expired(&self) -> bool {
        self.timestamp.elapsed() > self.ttl
    }
}

impl<T> Cache<Arc<[T]>> {
    pub fn get_cloned(&self) -> Option<Arc<[T]>> {
        self.get().cloned()
    }
}
