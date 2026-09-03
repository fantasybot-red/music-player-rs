use crate::players::tiktok::SigiState;
use anyhow::Result;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use wreq::Client as WreqClient;
use wreq_util::Emulation;

#[derive(Clone)]
pub struct TikTokClient {
    client: WreqClient,
    cookies: Arc<Mutex<HashMap<String, String>>>,
}

impl TikTokClient {
    pub fn new() -> Result<Self> {
        let client = WreqClient::builder()
            .emulation(Emulation::Chrome149)
            .build()
            .expect("Failed to create TikTok client");
        let cookies = Arc::new(Mutex::new(HashMap::new()));
        Ok(Self { client, cookies })
    }

    pub async fn fetch_stream_url(&self, username: &str) -> Result<SigiState> {
        let url = format!("https://www.tiktok.com/@{}/live", username);
        let response = self
            .client
            .get(&url)
            .header(
                "Cookie",
                self.cookies
                    .lock()
                    .await
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<String>>()
                    .join("; "),
            )
            .send()
            .await?;
        let body = response.text().await?;
        let dom = tl::parse(&body, tl::ParserOptions::default())?;
        let parser = dom.parser();
        let element_result = dom.get_element_by_id("SIGI_STATE");
        if element_result.is_none() {
            return Err(anyhow::anyhow!("Failed to find SIGI_STATE element"));
            // TODO: Handle the case where POW is required. This may involve solving a CAPTCHA or other challenge.
        }
        let inner_html = element_result
            .unwrap()
            .get(parser)
            .unwrap()
            .inner_html(parser);
        let sigi_state: SigiState = serde_json::from_str(&inner_html)?;
        Ok(sigi_state)
    }

    pub async fn check_stream(&self, url: &str) -> Result<bool> {
        let response = self.client.get(url).send().await?;
        Ok(response.status().is_success())
    }
}
