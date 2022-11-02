use frankenstein::User;

pub trait DisplayAt {
  fn to_at_string(self) -> String;
}

impl DisplayAt for Option<Box<User>> {
  fn to_at_string(self) -> String {
    self
      .and_then(|i| {
        let username = format!("@{}", i.clone().username?);
        let nickname = || {
          let space = if i.last_name.is_some() { " " } else { "" };
          let nickname = format!("{}{space}{}", i.first_name, i.last_name.unwrap_or_default(),);
          Some(nickname)
        };
        Some(username).or_else(nickname)
      })
      .unwrap_or_else(|| "Unknown".to_string())
  }
}
