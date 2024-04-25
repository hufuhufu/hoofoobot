use serde::{self, Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
pub struct AdminRecord {
    #[serde(flatten)]
    pub default: DefaultFields,
    pub email: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ScoreRecord {
    #[serde(flatten, skip_serializing)]
    pub default: DefaultFields,

    pub guild: String,
    pub player: String,
    pub voice_time: u64,
    pub afk_time: u64,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct PlayerRecord {
    #[serde(flatten, skip_serializing)]
    pub default: DefaultFields,

    pub user_id: String,
    pub username: String,
}

impl PlayerRecord {
    pub fn new(user_id: String) -> Self {
        PlayerRecord {
            user_id,
            ..Default::default()
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct GuildRecord {
    #[serde(flatten, skip_serializing)]
    pub default: DefaultFields,

    pub server_id: String,
    pub afk_channel: String,
    pub graveyard: String,
}

impl GuildRecord {
    pub fn new(server_id: String, afk_channel: Option<String>, graveyard: Option<String>) -> Self {
        GuildRecord {
            server_id,
            afk_channel: afk_channel.unwrap_or_default(),
            graveyard: graveyard.unwrap_or_default(),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultFields {
    pub id: String,
    pub created: String,
    pub updated: String,

    pub collection_id: Option<String>,
    pub collection_name: Option<String>,
}

pub trait Record {
    fn collection_name() -> &'static str;
    fn id(&self) -> &str;
}

macro_rules! impl_record {
    ($rec:ident, $coll_name: expr) => {
        impl Record for $rec {
            #[inline]
            fn collection_name() -> &'static str {
                $coll_name
            }
            fn id(&self) -> &str {
                self.default.id.as_str()
            }
        }
    };
}

impl_record!(GuildRecord, "guilds");
impl_record!(PlayerRecord, "players");
impl_record!(ScoreRecord, "scores");
