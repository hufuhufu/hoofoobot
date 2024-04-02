use tokio::sync::{mpsc, oneshot};

use crate::{
    pocketbase::client::{GuildRecord, PlayerRecord},
    score::GuildUser,
};

use super::client::{CVUResponse, Client, ListResponse, ScoreRecord};

pub type Responder<T> = oneshot::Sender<anyhow::Result<T>>;

macro_rules! bails {
    ($tx:expr, $err:expr $(,)?) => {
        let _ = $tx.send(anyhow::Result::Err($err));
        return;
    };
}

macro_rules! unwrap_result_or_bails {
    ($tx:expr, $result:expr) => {
        match $result {
            Ok(value) => value,
            Err(err) => {
                bails!($tx, err);
            }
        }
    };
}

macro_rules! do_list_or_bails {
    ($tx:expr, $res:expr, $res_type:ident, $blk:block) => {
        match $res {
            $res_type::Ok { .. } => $blk,
            $res_type::Error { error } => {
                bails!($tx, error.into());
            }
        }
    };
}

macro_rules! unwrap_record_or_bails {
    ($tx:expr, $res:expr) => {
        match $res {
            CVUResponse::Ok { record } => record,
            CVUResponse::Error { error } => {
                bails!($tx, error.into());
            }
        }
    };
}

pub enum Command {
    IncrScore {
        member: GuildUser,
        delta: u64,
        resp_tx: Responder<ScoreRecord>,
    },
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
        Command::IncrScore { .. } => incr_score_handler(client, cmd).await,
    };
}

async fn incr_score_handler(client: Client, cmd: Command) {
    let Command::IncrScore {
        member,
        delta,
        resp_tx,
    } = cmd;

    let filter = format!(
        "guild.server_id = \"{}\" && player.user_id = \"{}\"",
        member.0, member.1
    );

    let res = client
        .list::<ScoreRecord>("scores", Some(filter.as_str()))
        .await;
    let list_score_res = unwrap_result_or_bails!(resp_tx, res);

    do_list_or_bails!(resp_tx, list_score_res, ListResponse, {
        let mut items = list_score_res.unwrap();

        if !items.is_empty() {
            let mut score = items.pop().unwrap();
            score.voice_time += delta;

            let res = client.update::<ScoreRecord>(score).await;
            let record_res = unwrap_result_or_bails!(resp_tx, res);
            let record = unwrap_record_or_bails!(resp_tx, record_res);

            let _ = resp_tx.send(Ok(record));
        } else {
            let guild_record = {
                let filter = format!("server_id = \"{}\"", member.0);
                let res = client
                    .list::<GuildRecord>("guilds", Some(filter.as_str()))
                    .await;

                let list_guild_res = unwrap_result_or_bails!(resp_tx, res);

                do_list_or_bails!(resp_tx, list_guild_res, ListResponse, {
                    let mut items = list_guild_res.unwrap();

                    if !items.is_empty() {
                        items.pop().unwrap()
                    } else {
                        let guild_record = GuildRecord::new(member.0.to_string());
                        let res = client.create::<GuildRecord>("guilds", guild_record).await;
                        let record_res = unwrap_result_or_bails!(resp_tx, res);
                        unwrap_record_or_bails!(resp_tx, record_res)
                    }
                })
            };

            let player_record = {
                let filter = format!("server_id = \"{}\"", member.0);
                let res = client
                    .list::<PlayerRecord>("guilds", Some(filter.as_str()))
                    .await;

                let list_guild_res = unwrap_result_or_bails!(resp_tx, res);

                do_list_or_bails!(resp_tx, list_guild_res, ListResponse, {
                    let mut items = list_guild_res.unwrap();

                    if !items.is_empty() {
                        items.pop().unwrap()
                    } else {
                        let guild_record = PlayerRecord::new(member.0.to_string());
                        let res = client.create::<PlayerRecord>("guilds", guild_record).await;
                        let record_res = unwrap_result_or_bails!(resp_tx, res);
                        unwrap_record_or_bails!(resp_tx, record_res)
                    }
                })
            };

            let score_record = ScoreRecord {
                guild: guild_record.default.id.clone(),
                player: player_record.default.id.clone(),
                voice_time: delta,
                ..Default::default()
            };

            let res = client.create::<ScoreRecord>("scores", score_record).await;
            let record_res = unwrap_result_or_bails!(resp_tx, res);
            let record = unwrap_record_or_bails!(resp_tx, record_res);

            let _ = resp_tx.send(Ok(record));
        }
    });
}
