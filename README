# fuckburl-bot

Auto replace `b23.tv` to long url or removing tracking params of `bilibili.com/video/xxx` url.

## Build

```shell
cargo build --release
```

## Usage

```plaintext
Usage: fuckburl-bot [OPTIONS]

Options:
  -o, --config-file <DIR>
  -v, --verbose...         More output per occurrence
  -q, --quiet...           Less output per occurrence
  -h, --help               Print help information
```

You can run `fuckburl-bot` and a `config.toml` file will be generated in the working directory:

```toml
# Your telegram token, get from @BotFather
telegram-token = "139282332:fake_tokenlI_dAF41rNfFsaaa2EJvwi7qL91"
# Enabled groups, either name or id are supported
enabled-chats = ["group_name", "-10011231232"]

# # optional, proxy config, HTTP(S) and SOCKS5 are supported.
# proxy = "http://localhost:7899"

# [time]
# # fetch updates delay
# fetch-delay = 1000
# # fetch delay when last fetching failed
# failed-delay = 5000
```
