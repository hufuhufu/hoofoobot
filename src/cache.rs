use std::collections::HashMap;

use poise::serenity_prelude::GuildId;

use crate::database::Config;

#[derive(Default)]
pub struct DataCache {
    configs: HashMap<GuildId, Config>,
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
}
