use crate::config::{RadioStation, StationQuery};
use reqwest::Client;

fn codec_rank(codec: Option<&str>) -> u8 {
    match codec.map(|s| s.to_uppercase()).as_deref() {
        Some("FLAC")                          => 6,
        Some("AAC+")                          => 5,
        Some("AAC")                           => 4,
        Some("AAC+,H.264") | Some("AAC,H.264") => 4,
        Some("MP3")                           => 3,
        Some("OGG")                           => 2,
        Some("MP4") | Some("FLV")             => 1,
        _                                     => 0,
    }
}

pub async fn fetch_top_voted(
    client: &Client,
    limit: usize,
) -> Result<Vec<RadioStation>, reqwest::Error> {
    let mut stations = client
        .get(format!(
            "https://de1.api.radio-browser.info/json/stations/topvote/{limit}"
        ))
        .send()
        .await?
        .json::<Vec<RadioStation>>()
        .await?;

    stations.retain(|s| !s.name.is_empty() && !s.url.is_empty());
    Ok(stations)
}

pub async fn fetch_stations(
    client: &Client,
    query: StationQuery,
) -> Result<Vec<RadioStation>, reqwest::Error> {
    let mut params: Vec<(&str, String)> = Vec::new();

    if let Some(tag) = query.genre {
        params.push(("tagList", tag.to_lowercase()));
    }
    if let Some(country) = query.country {
        params.push(("countrycode", country));
    }
    if let Some(codec) = query.codec {
        params.push(("codec", codec));
    }
    if let Some(name) = query.name {
        params.push(("name", name));
    }
    if let Some(br) = query.min_bitrate {
        params.push(("bitrateMin", br.to_string()));
    }

    let mut stations = client
        .get("https://de1.api.radio-browser.info/json/stations/search")
        .query(&params)
        .send()
        .await?
        .json::<Vec<RadioStation>>()
        .await?;

    stations.retain(|s| !s.name.is_empty() && !s.url.is_empty());

    stations.sort_by(|a, b| {
        let rank_a = codec_rank(a.codec.as_deref());
        let rank_b = codec_rank(b.codec.as_deref());
        rank_b
            .cmp(&rank_a)
            .then_with(|| b.bitrate.unwrap_or(0).cmp(&a.bitrate.unwrap_or(0)))
    });

    Ok(stations)
}
