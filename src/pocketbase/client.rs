use std::{collections::HashMap, sync::Arc};

use anyhow::bail;
use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION},
    Url,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ListResponse<R: Record> {
    #[serde(rename_all = "camelCase")]
    Ok {
        page: u32,
        per_page: u32,
        total_items: u32,
        total_pages: u32,
        items: Vec<R>,
    },
    Error {
        #[serde(flatten)]
        error: ErrorResponse,
    },
}

impl<R: Record> ListResponse<R> {
    pub fn unwrap(self) -> Vec<R> {
        match self {
            ListResponse::Ok { items, .. } => items,
            ListResponse::Error { error } => {
                panic!("ListResponse called unwrap on Error: {}", error)
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum CVUResponse<R: Record> {
    Ok {
        #[serde(flatten)]
        record: R,
    },
    Error {
        #[serde(flatten)]
        error: ErrorResponse,
    },
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum AdminAuthResponse {
    Ok {
        token: String,
        admin: AdminRecord,
    },
    Error {
        #[serde(flatten)]
        error: ErrorResponse,
    },
}

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
    pub data: HashMap<String, Value>,
}

impl std::error::Error for ErrorResponse {}

impl core::fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PB_ERROR! code: {} message: {} data: {:?}",
            self.code, self.message, self.data
        )
    }
}

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
    pub fn new(server_id: String) -> Self {
        GuildRecord {
            server_id,
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
    fn collection_name(&self) -> &str;
    fn id(&self) -> &str;
}

macro_rules! impl_record {
    ($rec:ident) => {
        impl Record for $rec {
            fn collection_name(&self) -> &str {
                self.default
                    .collection_name
                    .as_ref()
                    .map(|x| x.as_str())
                    .unwrap_or_default()
            }
            fn id(&self) -> &str {
                self.default.id.as_str()
            }
        }
    };
}

impl_record!(GuildRecord);
impl_record!(PlayerRecord);
impl_record!(ScoreRecord);

#[derive(Clone, Debug)]
pub struct Client {
    pub reqwest_client: reqwest::Client,
    pub admin: Arc<AdminRecord>,
    pub pb_url: Arc<Url>,
}

impl Client {
    pub async fn new(pb_url: &str, username: &str, password: &str) -> anyhow::Result<Self> {
        let pb_url = reqwest::Url::parse(pb_url).expect("Failed to parse pocketbase url");
        let login_url = pb_url.join("/api/admins/auth-with-password")?;

        let login_client = reqwest::Client::new();
        let response = login_client
            .post(login_url)
            .json(&json!({"identity": username, "password": password}))
            .send()
            .await?;

        let resp = response.json::<AdminAuthResponse>().await?;

        match resp {
            AdminAuthResponse::Ok { token, admin } => {
                let mut auth_token = HeaderValue::from_str(&token)?;
                auth_token.set_sensitive(true);

                let mut headers = HeaderMap::with_capacity(1);
                headers.insert(AUTHORIZATION, auth_token);

                let client = reqwest::Client::builder()
                    .default_headers(headers)
                    .user_agent(APP_USER_AGENT)
                    .build()?;

                Ok(Client {
                    reqwest_client: client,
                    admin: admin.into(),
                    pb_url: pb_url.into(),
                })
            }
            AdminAuthResponse::Error { error } => {
                bail!(error)
            }
        }
    }

    pub async fn list<R: for<'de> Deserialize<'de> + Record>(
        &self,
        collection: &str,
        filter: Option<&str>,
    ) -> anyhow::Result<ListResponse<R>> {
        let url_path = format!("/api/collections/{collection}/records");
        let url = self.pb_url.join(&url_path)?;

        let req = self.reqwest_client.get(url.as_str());
        let req = match filter {
            Some(filter) => req.query(&[("filter", filter)]),
            None => req,
        };
        let res = req.send().await?;
        let list = res.json::<ListResponse<R>>().await?;

        Ok(list)
    }

    pub async fn view<R: for<'de> Deserialize<'de> + Serialize + Record>(
        &self,
        collection: &str,
        id: &str,
    ) -> anyhow::Result<CVUResponse<R>> {
        let url_path = format!("/api/collections/{collection}/records/{id}");
        let url = self.pb_url.join(&url_path)?;

        let req = self.reqwest_client.get(url);
        let res = req.send().await?.json::<CVUResponse<R>>().await?;

        Ok(res)
    }

    pub async fn create<R: for<'de> Deserialize<'de> + Serialize + Record>(
        &self,
        collection_name: &str,
        record: R,
    ) -> anyhow::Result<CVUResponse<R>> {
        let url_path = format!("/api/collections/{}/records", collection_name);
        let url = self.pb_url.join(&url_path)?;

        let req = self.reqwest_client.post(url).json::<R>(&record);
        let res = req.send().await?.json::<CVUResponse<R>>().await?;

        Ok(res)
    }

    pub async fn update<R: for<'de> Deserialize<'de> + Serialize + Record>(
        &self,
        record: R,
    ) -> anyhow::Result<CVUResponse<R>> {
        let url_path = format!(
            "/api/collections/{}/records/{}",
            record.collection_name(),
            record.id()
        );
        let url = self.pb_url.join(&url_path)?;

        let req = self.reqwest_client.patch(url.as_str()).json::<R>(&record);
        let res = req.send().await?.json::<CVUResponse<R>>().await?;

        Ok(res)
    }
}
