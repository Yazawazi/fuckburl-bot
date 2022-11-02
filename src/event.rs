use std::{fmt::Display, sync::Arc};

use anyhow::{Context, Ok, Result};
use frankenstein::{
  AsyncApi, AsyncTelegramApi, DeleteMessageParams, SendMessageParams, Update, UpdateContent,
};
use log::{debug, info};

use crate::{replacer::replace_all, util::DisplayAt, Config};

pub(crate) async fn process_update(
  api: &AsyncApi,
  config: Arc<Config>,
  update: Update,
) -> Result<()> {
  debug!("Processing update: {}", &update.update_id);
  match update.content {
    UpdateContent::Message(msg) => {
      if !config.enabled_chats.contains(&msg.chat.id.to_string()) {
        return Ok(());
      };

      let text = if let Some(text) = msg.text.clone() {
        text
      } else {
        return Ok(());
      };
      let replaced = replace_all(&*text)
        .await
        .context("Failed to replace text")?;
      if replaced == text {
        return Ok(());
      }

      info!("Replacing message {}", msg.chat.id);

      let mut send_msg = SendMessageParams::builder()
        .chat_id(msg.chat.id)
        .text(format!("Send by {}:\n{replaced}", msg.from.to_at_string()))
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
