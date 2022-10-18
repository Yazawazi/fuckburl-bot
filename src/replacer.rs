use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use regex::{Match, Regex};

lazy_static! {
  static ref BSHORT_REGEX: Regex =
    Regex::new(r"((https?://)?b23.tv/[0-9a-zA-Z]+/?)\??(?:&?[^=&]*=[^=&]*)*").unwrap();
  static ref BVIDEO_REGEX: Regex = Regex::new(
    r"(?P<url>(https?://)?(www\.)?bilibili.com/video/[0-9a-zA-Z]+/?)\??(?:&?[^=&]*=[^=&]*)*"
  )
  .unwrap();
}

pub fn replace_btrack(str: &str) -> String {
  BVIDEO_REGEX.replace_all(str, "$url").into()
}

pub async fn replace_bshort(str: &str) -> Result<String> {
  let matches = BSHORT_REGEX.find_iter(str);
  let matches_vec = matches.fold(Vec::new(), |mut acc: Vec<Match>, i| {
    acc.push(i);
    acc
  });
  let mut trim = String::from(str);
  let mut stream = stream::iter(matches_vec);
  while let Some(x) = stream.next().await {
    let url = x.as_str();
    trim = str.replace(url, &get_redirect_url(url).await?);
  }
  Ok(trim)
}

async fn get_redirect_url(url: &str) -> anyhow::Result<String> {
  let resp = reqwest::get(url)
    .await
    .with_context(|| format!("Failed to get url {url}"))?;
  let mut x = resp.url().clone();
  x.set_query(None);
  Ok(x.to_string())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test() {
    let result = replace_btrack("https://www.bilibili.com/video/BV1Hg411T7fT/?spm_id_from=333.788.recommend_more_video.1&vd_source=425ad7d352481d80617a03327da07da0");
    assert_eq!("https://www.bilibili.com/video/BV1Hg411T7fT/", result);
  }
}
