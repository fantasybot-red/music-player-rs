# Music Players (Rust Discord Bot)

A Discord music bot written in Rust 2024 using Serenity, Poise, and Songbird. It can queue Spotify and SoundCloud tracks and play audio from TikTok LIVE URLs.

## Features
- **Multiple sources:** Spotify tracks and playlists, SoundCloud tracks and playlists, and TikTok LIVE streams.
- **Interactive Spotify search:** Search terms return a Discord selection menu.
- **Playback controls:** Queue display, skipping, current-track looping, volume control, and now-playing details.
- **Spotify device login:** Bot owners can authenticate through the `dev sflogin` command.
- **Automatic disconnect:** The bot leaves when playback ends or no listeners remain in its voice channel.
- **Docker support:** The included multi-stage image installs FFmpeg in the runtime container.

## Prerequisites
- A Rust toolchain that supports edition 2024 (the Docker build uses Rust 1.98)
- Git (for fetching dependencies)
- FFmpeg (for media transcoding)
- `pkg-config`, `libssl-dev`, `cmake`, and `nasm` for building locally

## Discord Setup
1. Create a Discord App via the [Discord Developer Portal](https://discord.com/developers/applications).
2. Grab the bot token.
3. Enable **Message Content Intent** and **Server Members Intent** under the bot's privileged gateway intents. The application requests Discord's privileged intents as well as guild, message, and voice-state events.
4. Invite the bot with permission to view and send messages and to connect and speak in voice channels.

## Installation & Usage

### 1. Configure the Environment
Copy the template and fill in your values.
```sh
cp .env.template .env
```
Update `.env` with:
```env
TOKEN=<YOUR_BOT_TOKEN>
BOT_PREFIX=<YOUR_BOT_PREFIX>
CREDENTIALS_PATH=credentials.json
```
- `TOKEN`: Your Discord bot token.
- `BOT_PREFIX`: Optional text prefix used to invoke commands, such as `!` or `?`. Mentioning the bot also works as a prefix.
- `CREDENTIALS_PATH`: Path where Spotify OAuth credentials are read and written. Run the owner-only `dev sflogin` command to create this file before using Spotify playback or search.

### 2. Run Locally
Ensure `ffmpeg` is installed and accessible in your system's PATH.
```sh
cargo run --release
```

### 3. Run with Docker
The repository includes a two-stage Dockerfile that builds the Rust application and packages it in a minimalist image alongside FFmpeg and root certificates.
```sh
docker build -t rust-music-bot .
docker run --name rust-music-bot --env-file .env \
  -v "$(pwd)/credentials.json:/app/credentials.json" \
  -d rust-music-bot
```

Set `CREDENTIALS_PATH=/app/credentials.json` in `.env` when using the volume shown above. Create an empty `credentials.json` only if your container runtime requires the host-side mount target to exist; the bot writes valid credentials after `dev sflogin` completes.

## Commands
Default prefix is configurable via `BOT_PREFIX` (the bot also responds to mentions).

| Command      | Alias  | Description                             |
|--------------|--------|-----------------------------------------|
| `play <URL or query>` | `p` | Queues a supported URL or searches Spotify |
| `skip`       | `s`    | Skips the current track                 |
| `stop`       |        | Stops playback and disconnects bot      |
| `queue`      |        | Displays the current queue              |
| `volume <0-100>` | `vol` | Sets the current track's volume      |
| `loop [true\|false]` | `l` | Enables or disables looping for the current track |
| `nowplaying` | `np`   | Shows information on the current track  |

### Owner Only
- `dev sflogin`: Starts Spotify device authorization and returns a code and login URL. Poise determines bot ownership from Discord application information because owner initialization is enabled.

## Security Guidance
- Store your `TOKEN` securely.
- Never commit your `.env` or `credentials.json` to public repositories (both are included in `.gitignore`).
- The `dev` command hierarchy is strictly owner-only and cannot be executed by unauthorized server members.

## License
MIT License. See the [LICENSE](LICENSE) file for more details.
