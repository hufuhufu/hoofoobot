use poise::serenity_prelude::{ChannelId, GuildId};
use tokio::sync::{mpsc, oneshot};
use tracing::error;

use crate::{
    pocketbase::client::{CVUResponse, Client, ListResponse},
    pocketbase::records::{GuildRecord, PlayerRecord, ScoreRecord},
    score::{GuildUser, ScoreType},
};

pub type Responder<T> = oneshot::Sender<anyhow::Result<T>>;

macro_rules! bails {
    ($tx:expr, $err:expr) => {
        error!("Bails at {:?}", $err);
        let err = anyhow::anyhow!(
            "Sorry, something happened and your command can't be processed! Try again later!"
        );
        let _ = $tx.send(Err(err));
        return;
    };
}

macro_rules! unwrap_result_or_bails {
    ($tx:expr, $res:expr) => {
        match $res {
            Ok(value) => value,
            Err(err) => {
                bails!($tx, err);
            }
        }
    };
}

macro_rules! match_list_or_bails {
    ($tx:expr, $res:expr, $blk:block) => {
        match $res {
            ListResponse::Ok { .. } => $blk,
            ListResponse::Error { error } => {
                bails!($tx, error);
            }
        }
    };
}

macro_rules! unwrap_record_or_bails {
    ($tx:expr, $res:expr) => {
        match $res {
            CVUResponse::Ok { record } => record,
            CVUResponse::Error { error } => {
                bails!($tx, error);
            }
        }
    };
}

#[non_exhaustive]
pub enum Command {
    IncrScore(IncrScoreParams),
    SetConfig(SetConfigParams),
    GetConfig(GetConfigParams),
}

impl Command {
    pub fn new_incr_score(
        member: GuildUser,
        delta: u64,
        resp_tx: Responder<ScoreRecord>,
        score_type: ScoreType,
    ) -> Self {
        Self::IncrScore(IncrScoreParams {
            member,
            delta,
            resp_tx,
            score_type,
        })
    }

    pub fn new_set_config(
        guild_id: GuildId,
        afk_channel: Option<ChannelId>,
        graveyard: Option<ChannelId>,
        resp_tx: Responder<GuildRecord>,
    ) -> Self {
        Self::SetConfig(SetConfigParams {
            guild_id,
            afk_channel,
            graveyard,
            resp_tx,
        })
    }

    pub fn new_get_config(guild_id: GuildId, resp_tx: Responder<GuildRecord>) -> Self {
        Self::GetConfig(GetConfigParams { guild_id, resp_tx })
    }
}

pub struct IncrScoreParams {
    member: GuildUser,
    delta: u64,
    resp_tx: Responder<ScoreRecord>,
    score_type: ScoreType,
}

pub struct SetConfigParams {
    guild_id: GuildId,
    afk_channel: Option<ChannelId>,
    graveyard: Option<ChannelId>,
    resp_tx: Responder<GuildRecord>,
}

pub struct GetConfigParams {
    guild_id: GuildId,
    resp_tx: Responder<GuildRecord>,
}

pub struct Manager {
    pub client: Client,
}

impl Manager {
    pub fn new(client: Client) -> Self {
        Manager { client }
    }
    pub fn spawn(&self, mut rx: mpsc::Receiver<Command>) {
        let client = self.client.clone();
        tokio::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                command_handler(client.clone(), cmd).await;
            }
        });
    }
}

async fn command_handler(client: Client, cmd: Command) {
    match cmd {
        Command::IncrScore(param) => incr_score_handler(client, param).await,
        Command::SetConfig(param) => set_config_handler(client, param).await,
        Command::GetConfig(param) => get_config_handler(client, param).await,
    };
}

async fn incr_score_handler(client: Client, param: IncrScoreParams) {
    let IncrScoreParams {
        member,
        delta,
        resp_tx,
        score_type,
    } = param;

    let filter = format!(
        "guild.server_id = \"{}\" && player.user_id = \"{}\"",
        member.0, member.1
    );

    let res = client.list::<ScoreRecord>(Some(filter.as_str())).await;
    let list_score_res = unwrap_result_or_bails!(resp_tx, res);

    match_list_or_bails!(resp_tx, list_score_res, {
        let mut items = list_score_res.unwrap();

        if !items.is_empty() {
            let mut score = items.pop().unwrap();
            match score_type {
                ScoreType::Voice => score.voice_time += delta,
                ScoreType::Afk => score.afk_time += delta,
            }

            let res = client.update::<ScoreRecord>(score).await;
            let record_res = unwrap_result_or_bails!(resp_tx, res);
            let record = unwrap_record_or_bails!(resp_tx, record_res);

            let _ = resp_tx.send(Ok(record));
        } else {
            let guild_record = {
                let filter = format!("server_id = \"{}\"", member.0);
                let res = client.list::<GuildRecord>(Some(filter.as_str())).await;

                let list_guild_res = unwrap_result_or_bails!(resp_tx, res);

                match_list_or_bails!(resp_tx, list_guild_res, {
                    let mut items = list_guild_res.unwrap();

                    if !items.is_empty() {
                        items.pop().unwrap()
                    } else {
                        let guild_record = GuildRecord::new(member.0.to_string(), None, None);
                        let res = client.create::<GuildRecord>(guild_record).await;
                        let record_res = unwrap_result_or_bails!(resp_tx, res);
                        unwrap_record_or_bails!(resp_tx, record_res)
                    }
                })
            };

            let player_record = {
                let filter = format!("user_id = \"{}\"", member.1);
                let res = client.list::<PlayerRecord>(Some(filter.as_str())).await;

                let list_guild_res = unwrap_result_or_bails!(resp_tx, res);

                match_list_or_bails!(resp_tx, list_guild_res, {
                    let mut items = list_guild_res.unwrap();

                    if !items.is_empty() {
                        items.pop().unwrap()
                    } else {
                        let guild_record = PlayerRecord::new(member.0.to_string());
                        let res = client.create::<PlayerRecord>(guild_record).await;
                        let record_res = unwrap_result_or_bails!(resp_tx, res);
                        unwrap_record_or_bails!(resp_tx, record_res)
                    }
                })
            };

            let mut score_record = ScoreRecord {
                guild: guild_record.default.id.clone(),
                player: player_record.default.id.clone(),
                ..Default::default()
            };
            match score_type {
                ScoreType::Voice => score_record.voice_time += delta,
                ScoreType::Afk => score_record.afk_time += delta,
            }

            let res = client.create::<ScoreRecord>(score_record).await;
            let record_res = unwrap_result_or_bails!(resp_tx, res);
            let record = unwrap_record_or_bails!(resp_tx, record_res);

            let _ = resp_tx.send(Ok(record));
        }
    });
}

async fn set_config_handler(client: Client, param: SetConfigParams) {
    let SetConfigParams {
        guild_id,
        afk_channel,
        graveyard,
        resp_tx,
    } = param;

    let filter = format!("server_id = \"{}\"", guild_id);

    let res = client.list::<GuildRecord>(Some(&filter)).await;
    let list_guild_res = unwrap_result_or_bails!(resp_tx, res);

    match_list_or_bails!(resp_tx, list_guild_res, {
        let mut guilds = list_guild_res.unwrap();
        let afk_channel = afk_channel.as_ref().map(ChannelId::to_string);
        let graveyard = graveyard.as_ref().map(ChannelId::to_string);

        if !guilds.is_empty() {
            let mut guild = guilds.pop().unwrap();

            if let Some(ch) = afk_channel {
                guild.afk_channel = ch;
            }
            if let Some(ch) = graveyard {
                guild.graveyard = ch;
            }

            let res = client.update::<GuildRecord>(guild).await;
            let record_res = unwrap_result_or_bails!(resp_tx, res);
            let record = unwrap_record_or_bails!(resp_tx, record_res);

            let _ = resp_tx.send(Ok(record));
        } else {
            let guild = GuildRecord::new(guild_id.to_string(), afk_channel, graveyard);

            let res = client.create::<GuildRecord>(guild).await;
            let record_res = unwrap_result_or_bails!(resp_tx, res);
            let record = unwrap_record_or_bails!(resp_tx, record_res);

            let _ = resp_tx.send(Ok(record));
        };
    })
}

async fn get_config_handler(client: Client, param: GetConfigParams) {
    let GetConfigParams { guild_id, resp_tx } = param;

    let filter = format!("server_id = \"{}\"", guild_id);

    let res = client.list::<GuildRecord>(Some(&filter)).await;
    let list_guild_res = unwrap_result_or_bails!(resp_tx, res);

    match_list_or_bails!(resp_tx, list_guild_res, {
        let mut guilds = list_guild_res.unwrap();

        let record = if !guilds.is_empty() {
            guilds.pop().unwrap()
        } else {
            GuildRecord::new(guild_id.to_string(), None, None)
        };

        let _ = resp_tx.send(Ok(record));
    })
}
