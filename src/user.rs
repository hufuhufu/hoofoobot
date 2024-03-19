use std::sync::Arc;

use anyhow::Result;
use poise::serenity_prelude::UserId;

use crate::Context;

pub struct Username {
    username: Arc<str>,
}

impl Username {
    pub async fn from_user_id(ctx: Context<'_>, user_id: UserId) -> Result<Username> {
        let user = user_id.to_user(ctx).await?;
        let username: Arc<str> = user.name.into();

        Ok(Username { username })
    }
}

impl std::ops::Deref for Username {
    type Target = Arc<str>;

    fn deref(&self) -> &Self::Target {
        &self.username
    }
}
