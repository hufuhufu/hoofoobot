use std::{collections::HashMap, sync::Arc};

use anyhow::bail;
use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION},
    Url,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::records::{AdminRecord, Record};

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
    pub code: u16,
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
        filter: Option<&str>,
    ) -> anyhow::Result<ListResponse<R>> {
        let url_path = format!("/api/collections/{}/records", R::collection_name());
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
        id: &str,
    ) -> anyhow::Result<CVUResponse<R>> {
        let url_path = format!("/api/collections/{}/records/{}", R::collection_name(), id);
        let url = self.pb_url.join(&url_path)?;

        let req = self.reqwest_client.get(url);
        let res = req.send().await?.json::<CVUResponse<R>>().await?;

        Ok(res)
    }

    pub async fn create<R: for<'de> Deserialize<'de> + Serialize + Record>(
        &self,
        record: R,
    ) -> anyhow::Result<CVUResponse<R>> {
        let url_path = format!("/api/collections/{}/records", R::collection_name());
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
            R::collection_name(),
            record.id()
        );
        let url = self.pb_url.join(&url_path)?;

        let req = self.reqwest_client.patch(url.as_str()).json::<R>(&record);
        let res = req.send().await?.json::<CVUResponse<R>>().await?;

        Ok(res)
    }
}
