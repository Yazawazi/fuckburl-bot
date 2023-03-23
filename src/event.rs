use std::{fmt::Display, sync::Arc};

use anyhow::{Context, Ok, Result};
use frankenstein::{
  AsyncApi, AsyncTelegramApi, DeleteMessageParams, ParseMode, SendMessageParams, Update,
  UpdateContent, User,
};
use log::{debug, info};

use crate::{replacer::replace_all, Config, START_TIME};
use std::fmt::Write;

fn write_user(text: &mut String, user: &User) {
  match user.username {
    Some(ref at) => {
      write!(text, "@{at}").unwrap();
    },
    None => {
      write!(text, r#"<a href="tg://user?id={}">"#, user.id).unwrap();
      text.push_str(&v_htmlescape::escape(&user.first_name).to_string());
      if let Some(ref last) = user.last_name {
        write!(text, " {}", v_htmlescape::escape(last)).unwrap();
      }
      text.push_str("</a>");
    },
  }
}

pub(crate) async fn process_update(
  api: &AsyncApi,
  config: Arc<Config>,
  update: Update,
) -> Result<()> {
  debug!("Processing update: {}", &update.update_id);
  match update.content {
    UpdateContent::Message(msg) => {
      if msg.date < *START_TIME {
        return Ok(());
      }
      if !config.enabled_chats.contains(&msg.chat.id.to_string()) {
        return Ok(());
      };

      let text = if let Some(text) = msg.text.clone() {
        text
      } else {
        return Ok(());
      };
      let replaced = replace_all(&text).await.context("Failed to replace text")?;
      if replaced == text {
        return Ok(());
      }

      info!("Replacing message {}", msg.chat.id);

      let mut text = String::with_capacity(128);
      write!(text, "Send by ").unwrap();
      match msg.from {
        Some(user) => write_user(&mut text, &user),
        None => {
          write!(text, "Unknown").unwrap();
        },
      }

      if let Some(from) = msg.forward_from {
        text.write_str(", forwarded from ").unwrap();
        write_user(&mut text, &from);
      } else if let Some(from_chat) = msg.forward_from_chat {
        text.write_str(", forwarded from channel ").unwrap();
        let title = from_chat
          .title
          .map(|title| v_htmlescape::escape(&title).to_string())
          .unwrap_or_else(|| "unknown".to_string());
        if let (Some(username), Some(msg_id)) = (from_chat.username, msg.forward_from_message_id) {
          write!(
            text,
            r#"<a href="https://t.me/{username}/{msg_id}">{title}</a>"#,
          )
          .unwrap();
        } else if let Some(msg_id) = msg.forward_from_message_id {
          debug!("from_chat.id = {}", from_chat.id);
          let id = -(from_chat.id + 1000000000000);
          write!(
            text,
            r#"<a href="https://t.me/c/{id}/{msg_id}">{title}</a>"#,
          )
          .unwrap();
        } else {
          text.write_str(&title).unwrap();
        }
      } else if let Some(ref sender_name) = msg.forward_sender_name {
        write!(
          text,
          ", forwarded from {}",
          v_htmlescape::escape(sender_name)
        )
        .unwrap();
      }

      writeln!(text, ":").unwrap();

      text.push_str(&v_htmlescape::escape(&replaced).to_string());

      let mut send_msg = SendMessageParams::builder()
        .chat_id(msg.chat.id)
        .text(text)
        .parse_mode(ParseMode::Html)
        .build();

      send_msg.reply_to_message_id = msg.reply_to_message.map(|i| i.message_id);

      let resp = api
        .send_message(&send_msg)
        .await
        .context("Failed to send message...")?;
      debug!("{resp:?}");

      let resp = api
        .delete_message(
          &DeleteMessageParams::builder()
            .chat_id(msg.chat.id)
            .message_id(msg.message_id)
            .build(),
        )
        .await
        .context("Failed to delete message...")?;
      debug!("{resp:?}",);

      Ok(())
    },
    _ => {
      info!("Unsupported message type: {}", MessageType(update.content));
      Ok(())
    },
  }
}

struct MessageType(UpdateContent);

impl Display for MessageType {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let str = match self.0 {
      UpdateContent::Message(_) => "Message",
      UpdateContent::EditedMessage(_) => "EditedMessage",
      UpdateContent::ChannelPost(_) => "ChannelPost",
      UpdateContent::EditedChannelPost(_) => "EditedChannelPost",
      UpdateContent::InlineQuery(_) => "InlineQuery",
      UpdateContent::ChosenInlineResult(_) => "ChosenInlineResult",
      UpdateContent::CallbackQuery(_) => "CallbackQuery",
      UpdateContent::ShippingQuery(_) => "ShippingQuery",
      UpdateContent::PreCheckoutQuery(_) => "PreCheckoutQuery",
      UpdateContent::Poll(_) => "Poll",
      UpdateContent::PollAnswer(_) => "PollAnswer",
      UpdateContent::MyChatMember(_) => "MyChatMember",
      UpdateContent::ChatMember(_) => "ChatMember",
      UpdateContent::ChatJoinRequest(_) => "ChatJoinRequest",
    };
    f.write_str(str)
  }
}
