use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RadioStation {
    pub name: String,
    pub url: String,
    pub country: Option<String>,
    #[serde(rename = "tags")]
    pub genre: Option<String>,
    pub bitrate: Option<u32>,
    pub codec: Option<String>,
}

pub struct StationQuery {
    pub name: Option<String>,
    pub genre: Option<String>,
    pub country: Option<String>,
    pub codec: Option<String>,
    pub min_bitrate: Option<u32>,
}
