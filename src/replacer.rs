use std::{
  borrow::{Borrow, Cow},
  str::FromStr,
};

use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use regex::{Match, Regex};
use reqwest::Url;

lazy_static! {
  static ref BSHORT_REGEX: Regex =
    Regex::new(r"((https?://)?b23.tv/[0-9a-zA-Z]+/?)\??(?:&?[^=&]*=[^=&]*)*").unwrap();
  static ref BVIDEO_REGEX: Regex = Regex::new(
    r"(?P<url>(https?://)?(www\.)?bilibili.com/video/[0-9a-zA-Z]+/?)\??(?:&?[^=&]*=[^=&]*)*"
  )
  .unwrap();
  static ref BARTICLE_REGEX: Regex = Regex::new(
    r"(https?://)?(www\.)?bilibili.com/read/mobile/(?P<cvid>[0-9]+)\??(?:&?[^=&]*=[^=&]*)*"
  )
  .unwrap();
  static ref AMAZON_REGEX: Regex = Regex::new(
    r"(?P<domain>(https?://)?(www\.)?amazon.com/)[a-zA-Z0-9%-]+/(?P<path>dp/[0-9a-zA-Z]+/?)\??(?:&?[^=&]*=[^=&]*)*"
  ).unwrap();
  static ref AMAZON_SEARCH_REGEX: Regex = Regex::new(
    r"(?P<domain>(https?://)?(www\.)?amazon.com/s)(?P<keyword>\?k=[a-zA-Z0-9%+-]+)(?:&?[^=&]*=[^=&]*)*"
  )
  .unwrap();
}

pub async fn replace_all(text: &str) -> Result<String> {
  let mut new = text.to_string();
  new = replace_bshort(&*new).await.context("Failed to replace short url")?;
  new = replace_btrack(&*new);
  new = replace_barticle(&*new);
  new = replace_amazon(&*new);
  new = replace_amazon_search(&*new);
  Ok(new)
}

fn replace_amazon(url: &str) -> String {
  AMAZON_REGEX.replace_all(url, "$domain$path").into()
}

fn replace_amazon_search(url: &str) -> String {
  AMAZON_SEARCH_REGEX
    .replace_all(url, "$domain$keyword")
    .into()
}

fn trim_bili_link(url: &mut Url) {
  const KEYS: Cow<[&str]> = Cow::Borrowed(&["p", "t"]);
  url.keep_pairs_only_in(KEYS);
}

fn replace_btrack(str: &str) -> String {
  let mut new_str = str.to_string();
  for i in BVIDEO_REGEX.find_iter(str.borrow()) {
    let mut url = if let Ok(url) = Url::from_str(i.as_str()) {
      url
    } else {
      continue;
    };
    trim_bili_link(&mut url);
    new_str.replace_range(i.range(), url.to_string().as_str());
  }
  new_str
}

async fn replace_bshort(str: &str) -> Result<String> {
  let mut new_str = str.to_string();
  let matches: Vec<Match> = BSHORT_REGEX.find_iter(str).collect();
  let mut stream = stream::iter(matches);
  while let Some(x) = stream.next().await {
    let mut url = get_redirect_url(x.as_str()).await?;
    trim_bili_link(&mut url);
    new_str.replace_range(x.range(), url.to_string().as_str());
  }
  Ok(new_str)
}

fn replace_barticle(str: &str) -> String {
  BARTICLE_REGEX.replace_all(str, "https://www.bilibili.com/read/cv$cvid").into()
}

async fn get_redirect_url(url: &str) -> anyhow::Result<Url> {
  let resp = reqwest::get(url)
    .await
    .with_context(|| format!("Failed to get url {url}"))?;
  Ok(resp.url().clone())
}

trait RemovePairsIf {
  fn remove_pairs_if_key<P>(&mut self, predicate: P)
  where
    Self: Sized,
    P: Fn(&str) -> bool;

  #[inline]
  fn keep_pairs_only_in(&mut self, vec: Cow<[&str]>)
  where
    Self: Sized,
  {
    self.remove_pairs_if_key(|k| !vec.contains(&k.borrow()));
  }
}

impl RemovePairsIf for Url {
  #[inline]
  fn remove_pairs_if_key<P>(&mut self, predicate: P)
  where
    Self: Sized,
    P: Fn(&str) -> bool,
  {
    let buf = String::new();
    let mut ser = form_urlencoded::Serializer::new(buf);
    self.query_pairs().into_iter().for_each(|(k, v)| {
      if !predicate(k.borrow()) {
        ser.append_pair(k.borrow(), v.borrow());
      }
    });

    self.set_query(match &*ser.finish() {
      "" => None,
      query @ _ => Some(query),
    });
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn remove_all() {
    let result = replace_btrack("https://www.bilibili.com/video/BV1Hg411T7fT/?spm_id_from=333.788.recommend_more_video.1&vd_source=425ad7d352481d80617a03327da07da0");
    assert_eq!("https://www.bilibili.com/video/BV1Hg411T7fT/", result);
  }

  #[test]
  fn keep_certain_params() {
    assert_eq!(
      "https://www.bilibili.com/video/BV114514/?t=123&p=1",
      replace_btrack("https://www.bilibili.com/video/BV114514/?t=123&p=1&spm=1.2212.22321")
    );
    assert_eq!(
      "https://www.bilibili.com/video/BV114514/?t=123",
      replace_btrack("https://www.bilibili.com/video/BV114514/?t=123&spm=1.2212.22321")
    );
  }

  #[tokio::test]
  async fn bshort() {
    assert_eq!(
      "https://www.bilibili.com/video/BV1se4y177g9/?t=100",
      replace_bshort("https://b23.tv/lBI8Ov3").await.unwrap(),
    );
  }

  #[test]
  fn amazon() {
    assert_eq!(
      "https://www.amazon.com/dp/B00NLZUM36/",
      replace_amazon("https://www.amazon.com/Redragon-S101-Keyboard-Ergonomic-Programmable/dp/B00NLZUM36/ref=sr_1_1?keywords=gaming+keyboard&pd_rd_r=89c237af-e7f2-4af6-b9c4&pd_rd_w=0aaaD&pd_rd_wg=KZWal&pf_rd_p=112312321&pf_rd_r=1233&qid=234231231&qu=eyJxc2MiOinFzcCI6IjYuMjAifQ%3D%3D&sr=8-1"),
    )
  }

  #[test]
  fn amazon_search() {
    assert_eq!(
      "https://www.amazon.com/s?k=%E4%BD%A0%E5%A5%BD%26+%2B",
      replace_amazon_search("https://www.amazon.com/s?k=%E4%BD%A0%E5%A5%BD%26+%2B&crid=1SHSKHE0RZCED&sprefix=%E4%BD%A0%E5%A5%BD%26+%2B%2Caps%2C1307&ref=nb_sb_noss_2")
    )
  }

  #[test]
  fn replace_barticle_test() {
    assert_eq!(
      "https://www.bilibili.com/read/cv19172625",
      replace_barticle("https://www.bilibili.com/read/mobile/19172625?xxx=114514&asdfasdf=32394239ADSAD-12312aASDASD")
    )
  }
}
