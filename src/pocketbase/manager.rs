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
                handle_command(client.clone(), cmd).await;
            }
        });
    }
}

async fn handle_command(client: Client, cmd: Command) {
    match cmd {
        Command::IncrScore {
            member,
            delta,
            resp_tx,
        } => {
            let filter = format!(
                "guild.server_id = \"{}\" && player.user_id = \"{}\"",
                member.0, member.1
            );

            let res = client
                .list::<ScoreRecord>("scores", Some(filter.as_str()))
                .await;
            let list = match res {
                Ok(list) => list,
                Err(err) => {
                    bails!(resp_tx, err);
                }
            };

            match list {
                ListResponse::Ok { mut items, .. } => {
                    if !items.is_empty() {
                        let mut score = items.pop().unwrap();
                        score.voice_time += delta;

                        let res = client.update::<ScoreRecord>(score).await;
                        let res = match res {
                            Ok(record) => record,
                            Err(err) => {
                                bails!(resp_tx, err);
                            }
                        };
                        let record = match res {
                            CVUResponse::Ok { record } => Ok(record),
                            CVUResponse::Error { error } => {
                                bails!(resp_tx, error.into());
                            }
                        };

                        let _ = resp_tx.send(record);
                    } else {
                        let guild_record = {
                            let filter = format!("server_id = \"{}\"", member.0);
                            let res = client
                                .list::<GuildRecord>("guilds", Some(filter.as_str()))
                                .await;
                            let list = match res {
                                Ok(list) => list,
                                Err(err) => {
                                    bails!(resp_tx, err);
                                }
                            };
                            match list {
                                ListResponse::Ok { mut items, .. } => {
                                    if !items.is_empty() {
                                        items.pop().unwrap()
                                    } else {
                                        let guild_record = GuildRecord {
                                            server_id: member.0.to_string(),
                                            ..Default::default()
                                        };
                                        let res = client
                                            .create::<GuildRecord>("guilds", guild_record)
                                            .await;
                                        let res = match res {
                                            Ok(record) => record,
                                            Err(err) => {
                                                bails!(resp_tx, err);
                                            }
                                        };
                                        match res {
                                            CVUResponse::Ok { record } => record,
                                            CVUResponse::Error { error } => {
                                                bails!(resp_tx, error.into());
                                            }
                                        }
                                    }
                                }
                                ListResponse::Error { error } => {
                                    bails!(resp_tx, error.into());
                                }
                            }
                        };

                        let player_record = {
                            let filter = format!("user_id = \"{}\"", member.1);
                            let res = client
                                .list::<PlayerRecord>("players", Some(filter.as_str()))
                                .await;
                            let list = match res {
                                Ok(list) => list,
                                Err(err) => {
                                    bails!(resp_tx, err);
                                }
                            };
                            match list {
                                ListResponse::Ok { mut items, .. } => {
                                    if !items.is_empty() {
                                        items.pop().unwrap()
                                    } else {
                                        let player_record = PlayerRecord {
                                            user_id: member.1.to_string(),
                                            ..Default::default()
                                        };
                                        let res = client
                                            .create::<PlayerRecord>("players", player_record)
                                            .await;
                                        let res = match res {
                                            Ok(record) => record,
                                            Err(err) => {
                                                bails!(resp_tx, err);
                                            }
                                        };
                                        match res {
                                            CVUResponse::Ok { record } => record,
                                            CVUResponse::Error { error } => {
                                                bails!(resp_tx, error.into());
                                            }
                                        }
                                    }
                                }
                                ListResponse::Error { error } => {
                                    bails!(resp_tx, error.into());
                                }
                            }
                        };

                        let score_record = ScoreRecord {
                            guild: guild_record.default.id.clone(),
                            player: player_record.default.id.clone(),
                            voice_time: delta,
                            ..Default::default()
                        };

                        let res = client.create::<ScoreRecord>("scores", score_record).await;
                        let res = match res {
                            Ok(res) => res,
                            Err(err) => {
                                bails!(resp_tx, err);
                            }
                        };
                        let record = match res {
                            CVUResponse::Ok { record } => Ok(record),
                            CVUResponse::Error { error } => {
                                bails!(resp_tx, error.into());
                            }
                        };

                        let _ = resp_tx.send(record);
                    }
                }
                ListResponse::Error { error } => {
                    bails!(resp_tx, error.into());
                }
            };
        }
    }
}
