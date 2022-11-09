#[macro_use]
extern crate lazy_static;

mod event;
mod replacer;
mod util;

use async_stream::stream;
use futures::pin_mut;
use futures_util::stream::StreamExt;
use log::{debug, error, info, LevelFilter};
use log4rs::{
  append::console::ConsoleAppender,
  config::{Appender, Root},
  encode::pattern::PatternEncoder,
};
use reqwest::{Client, Proxy};
use serde::Deserialize;

use std::{
  fs::{self, File},
  io::{BufReader, BufWriter, Read, Write},
  path::PathBuf,
  process,
  sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
  },
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};
use clap::{Parser, ValueHint};
use clap_verbosity_flag::{LogLevel, Verbosity};
use frankenstein::{AllowedUpdate, AsyncApi, AsyncTelegramApi, GetUpdatesParams};

use crate::event::process_update;

#[derive(Parser, Debug)]
struct Cli {
  #[arg(short = 'c', long, value_name = "DIR")]
  #[arg(value_hint = ValueHint::FilePath)]
  config_file: Option<PathBuf>,
  #[clap(flatten)]
  verbose: Verbosity<DefaultLevel>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "kebab-case"))]
struct Config {
  telegram_token: String,
  #[serde(default = "Default::default")]
  enabled_chats: Vec<String>,
  proxy: Option<String>,
  #[serde(default = "Default::default")]
  time: Time,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "kebab-case"))]
struct Time {
  fetch_delay: u64,
  failed_delay: u64,
}

impl Default for Time {
  fn default() -> Self {
    Self {
      fetch_delay: 1000,
      failed_delay: 5000,
    }
  }
}

lazy_static! {
  static ref START_TIME: u64 = {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    since_the_epoch.as_secs()
  };
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
  let args = Cli::parse();
  init_logger(args.verbose.log_level_filter());
  info!("Start at: {:?}", *START_TIME);
  debug!("{args:?}");
  let config = init_config(args.config_file).context("Failed to init config file")?;
  let config = Arc::new(config);
  debug!("{config:?}");

  let mut cli = Client::builder();
  if let Some(proxy) = &config.proxy {
    let proxy =
      Proxy::all(proxy.clone()).with_context(|| format!("Failed to set \"{proxy}\" as proxy"))?;
    cli = cli.proxy(proxy);
  }
  let cli = cli.build()?;

  let tg_api = AsyncApi::builder()
    .api_url(format!(
      "{}{}",
      frankenstein::BASE_API_URL,
      &*config.telegram_token,
    ))
    .client(cli.clone())
    .build();
  let tg_api = Arc::new(tg_api);
  let me = tg_api
    .get_me()
    .await
    .context("Failed to get telegram bot self info")?;
  info!(
    "Current tg bot: {}",
    me.result
      .username
      .context("Failed to get username for bot, maybe token is invalid")?
  );

  let update_seq = AtomicU32::new(0);

  fn update_params(offset: u32) -> GetUpdatesParams {
    GetUpdatesParams::builder()
      .allowed_updates(vec![AllowedUpdate::Message])
      .offset(offset)
      .limit(500u32)
      .build()
  }

  let stream = {
    let tg_api = Arc::clone(&tg_api);
    let config = Arc::clone(&config);
    stream! {
      loop {
        let result = tg_api.get_updates(&update_params(update_seq.load(Ordering::Acquire))).await;
        let updates = match result {
          Ok(msg) => msg.result,
          Err(err) => {
            error!(
              "Failed to get updates, retry after {}ms: {:?}",
              config.time.failed_delay,
              err.to_string()
            );
            tokio::time::sleep(Duration::from_millis(config.time.failed_delay)).await;
            continue;
          },
        };
        if let Some(last) = updates.iter().last() {
          let new_id = last.update_id + 1;
          update_seq.store(new_id, Ordering::Release);
        }
        for update in updates.into_iter() {
          yield update;
        }
        debug!("Yield updates..");
        tokio::time::sleep(Duration::from_millis(config.time.fetch_delay)).await;
      }
    }
  };

  pin_mut!(stream);

  while let Some(value) = stream.next().await {
    let tg_api = Arc::clone(&tg_api);
    let config = Arc::clone(&config);
    tokio::spawn(async move {
      if let Err(err) = process_update(&tg_api, config, value).await {
        error!("Error during processing update: {err}")
      };
    });
  }

  Ok(())
}

#[cfg(debug_assertions)]
type DefaultLevel = DebugLevel;

#[cfg(not(debug_assertions))]
type DefaultLevel = clap_verbosity_flag::InfoLevel;

#[derive(Copy, Clone, Debug, Default)]
pub struct DebugLevel;

impl LogLevel for DebugLevel {
  fn default() -> Option<log::Level> {
    Some(log::Level::Debug)
  }
}

fn init_logger(verbosity: LevelFilter) {
  const PATTERN: &str = "{d(%m-%d %H:%M)} {h({l:.1})} - {h({m})}{n}";
  let stdout = ConsoleAppender::builder()
    .encoder(Box::new(PatternEncoder::new(PATTERN)))
    .build();
  let config = log4rs::Config::builder()
    .appender(Appender::builder().build("stdout", Box::new(stdout)))
    .build(Root::builder().appender("stdout").build(verbosity))
    .unwrap();
  log4rs::init_config(config).unwrap();
}

fn init_config(path: Option<PathBuf>) -> Result<Config> {
  let path = if let Some(dir) = path {
    dir
  } else if cfg!(debug_assertions) {
    std::env::current_dir()
      .context("Failed to get current dir")?
      .join("work_dir")
      .join("config.toml")
  } else {
    std::env::current_dir()
      .context("Failed to get current dir")?
      .join("config.toml")
  };

  info!("Initializing config file...");

  if path.exists() && path.is_file() {
    info!("Reading config from {}...", &path.to_string_lossy());
    let file = File::open(&path).context("Failed to")?;
    let mut buf_reader = BufReader::new(file);
    let mut config_str = String::new();
    buf_reader
      .read_to_string(&mut config_str)
      .with_context(|| {
        format!(
          "Failed to read config file as String: {}",
          &path.to_string_lossy()
        )
      })?;
    let config: Config = toml::from_str(&*config_str)
      .with_context(|| format!("Failed to parse config file: {}", &path.to_string_lossy()))?;
    Ok(config)
  } else if !path.exists() {
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent)
        .with_context(|| format!("Failed to create folder: {}", parent.to_string_lossy()))?;
    };
    let config = File::create(&path).with_context(|| {
      format!(
        "Failed to create default config: {}",
        &path.to_string_lossy()
      )
    })?;
    const DEFAULT_CONFIG: &[u8] = include_bytes!("config.example.toml");

    {
      let mut buf_writer = BufWriter::new(config);
      buf_writer.write_all(DEFAULT_CONFIG).with_context(|| {
        format!(
          "Failed to write default config to: {}",
          &path.to_string_lossy()
        )
      })?;
    }
    info!("Default config writed to {}", &path.to_string_lossy());
    info!("Please take a look and configure bot, exiting...");
    process::exit(0)
  } else {
    bail!("Path is not a file: {}", path.to_string_lossy())
  }
}
