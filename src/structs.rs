use std::sync::Arc;

use regex::Regex;
use lazy_static::lazy_static;

use lavalink_rs::LavalinkClient;
use poise::serenity_prelude as serenity;

use crate::constants::RED;

#[derive(serde::Deserialize)]
pub struct Config {
    #[serde(rename="Main")] pub main: MainConfig,
    #[serde(rename="Lavalink")] pub lavalink: LavalinkConfig,
    #[serde(rename="Webhook-Info")] pub webhooks: toml::value::Table,
}

#[derive(serde::Deserialize)]
pub struct MainConfig {
    pub translation_token: Option<String>,
    pub patreon_role: serenity::RoleId,
    pub main_server: serenity::GuildId,
    pub main_server_invite: String,
    pub tts_service: reqwest::Url,
    pub token: Option<String>,
    pub invite_channel: u64,
    pub log_level: String,
    pub ofs_role: u64,
}

#[derive(serde::Deserialize)]
pub struct PostgresConfig {
    pub host: String,
    pub user: String,
    pub database: String,
    pub password: String,
}

#[derive(serde::Deserialize)]
pub struct LavalinkConfig {
    pub password: String,
    pub host: String,
    pub port: u16,
    pub ssl: bool
}


pub struct Data {
    pub analytics: Arc<crate::analytics::Handler>,
    pub guilds_db: crate::database::Handler<i64>,
    pub userinfo_db: crate::database::Handler<i64>,
    pub nickname_db: crate::database::Handler<[i64; 2]>,
    pub user_voice_db: crate::database::Handler<(i64, TTSMode)>,
    pub guild_voice_db: crate::database::Handler<(i64, TTSMode)>,

    pub webhooks: std::collections::HashMap<String, serenity::Webhook>,
    pub last_to_xsaid_tracker: LastToXsaidTracker,
    pub premium_users: Vec<serenity::UserId>,
    pub start_time: std::time::SystemTime,
    pub premium_avatar_url: String,
    pub lavalink: LavalinkClient,
    pub reqwest: reqwest::Client,
    pub config: MainConfig,

    pub premium_voices: PremiumVoices,
    pub pool: Arc<deadpool_postgres::Pool>,
}


#[derive(
    postgres_types::ToSql, postgres_types::FromSql,
    Debug, Hash, PartialEq, Eq, Copy, Clone
)]
#[postgres(name="ttsmode")]
pub enum TTSMode {
    #[postgres(name="gtts")] Gtts,
    #[postgres(name="espeak")] Espeak,
    #[postgres(name="premium")] Premium
}

impl Default for TTSMode {
    fn default() -> Self {
        Self::Gtts
    }
}

impl std::fmt::Display for TTSMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Gtts => "gTTS",
            Self::Espeak => "eSpeak",
            Self::Premium => "Premium"
        })
    }
}

#[derive(poise::ChoiceParameter)]
pub enum TTSModeServerChoice {
    // Name to show in slash command invoke           Aliases for prefix
    #[name="Google Translate TTS (female) (default)"] #[name="gtts"]       Gtts,
    #[name="eSpeak TTS (male)"]                       #[name="espeak"]     Espeak,
    #[name="Premium TTS (changable)"]                 #[name="premium"]    Premium
}

#[derive(poise::ChoiceParameter)]
pub enum TTSModeChoice {
    // Name to show in slash command invoke           Aliases for prefix
    #[name="Google Translate TTS (female) (default)"] #[name="gtts"]       Gtts,
    #[name="eSpeak TTS (male)"]                       #[name="espeak"]     Espeak,
}

impl From<TTSModeServerChoice> for TTSMode {
    fn from(mode: TTSModeServerChoice) -> Self {
        match mode {
            TTSModeServerChoice::Gtts => TTSMode::Gtts,
            TTSModeServerChoice::Espeak => TTSMode::Espeak,
            TTSModeServerChoice::Premium => TTSMode::Premium
        }
    }
}

impl From<TTSModeChoice> for TTSMode {
    fn from(mode: TTSModeChoice) -> Self {
        match mode {
            TTSModeChoice::Gtts => TTSMode::Gtts,
            TTSModeChoice::Espeak => TTSMode::Espeak,
        }
    }
}

#[derive(serde::Deserialize, Debug)]
pub struct DeeplTranslateResponse {
    pub translations: Vec<DeeplTranslation>
}

#[derive(serde::Deserialize, Debug)]
pub struct DeeplTranslation {
    pub text: String,
    pub detected_source_language: String
}

#[derive(serde::Deserialize, Debug)]
pub struct DeeplVoice {
    pub language: String,
}

#[allow(non_snake_case)]
#[derive(serde::Deserialize, Debug)]
pub struct GoogleVoice<'a> {
    pub name: String,
    pub ssmlGender: &'a str,
    pub languageCodes: [String; 1],
}

#[derive(serde::Serialize, Debug)]
pub enum Gender {
    Male,
    Female
}

impl std::fmt::Display for Gender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Gender::Male => "Male",
            Gender::Female => "Female"
        })
    }
}


#[derive(Debug)]
pub enum Error {
    GuildOnly,
    None(String),
    DebugLog(&'static str), // debug log something but ignore
    Unexpected(Box<dyn std::error::Error + Send + Sync>),
}

impl<E> From<E> for Error
where
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    fn from(e: E) -> Self {
        Self::Unexpected(e.into())
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

pub type Context<'a> = poise::Context<'a, Data, Error>;
pub type PremiumVoices = std::collections::BTreeMap<String, std::collections::BTreeMap<String, Gender>>;
pub type LastToXsaidTracker = dashmap::DashMap<serenity::GuildId, (serenity::UserId, std::time::SystemTime)>;

pub trait OptionTryUnwrap<T> {
    fn try_unwrap(self) -> Result<T, Error>;
}

#[serenity::async_trait]
pub trait PoiseContextAdditions {
    async fn author_permissions(&self) -> Result<serenity::Permissions, Error>;
    async fn send_error(&self, error: &str, fix: Option<String>) -> Result<Option<poise::ReplyHandle<'_>>, Error>;
}
#[serenity::async_trait]
pub trait SerenityContextAdditions {
    async fn user_from_dm(&self, dm_name: &str) -> Option<serenity::User>;
    async fn join_vc(
        &self,
        lavalink: &LavalinkClient,
        guild_id: serenity::GuildId,
        channel_id: serenity::ChannelId,
    ) -> Result<(), Error>;
}

#[serenity::async_trait]
impl PoiseContextAdditions for Context<'_> {
    async fn author_permissions(&self) -> Result<serenity::Permissions, Error> {
        let ctx_discord = self.discord();
        
        match ctx_discord.cache.channel(self.channel_id()).ok_or("author perms no channel")? {
            serenity::Channel::Guild(channel) => {
                let guild = channel.guild(&ctx_discord.cache).ok_or("author perms no guild")?;
                let member = guild.member(ctx_discord, self.author()).await?;

                Ok(guild.user_permissions_in(&channel, &member)?)
            }
            _ => {
                Ok(((serenity::Permissions::from_bits_truncate(0b111110010000000000001111111100001000000)
                    | serenity::Permissions::SEND_MESSAGES)
                    - serenity::Permissions::SEND_TTS_MESSAGES)
                    - serenity::Permissions::MANAGE_MESSAGES)
            }
        }
    }
    async fn send_error(&self, error: &str, fix: Option<String>) -> Result<Option<poise::ReplyHandle<'_>>, Error> {
        let author = self.author();
        let fix =
            fix.unwrap_or_else(|| String::from("get in contact with us via the support server"));

        let ctx_discord = self.discord();
        let (name, avatar_url) = match self.channel_id().to_channel(ctx_discord).await? {
            serenity::Channel::Guild(channel) => {
                let permissions = channel.permissions_for_user(ctx_discord, ctx_discord.cache.current_user_id())?;

                if !permissions.send_messages() {
                    return Ok(None);
                };

                if !permissions.embed_links() {
                    self.send(|b| {
                        b.ephemeral(true);
                        b.content("An Error Occurred! Please give me embed links permissions so I can tell you more!")
                    }).await?;
                    return Ok(None);
                };

                match channel.guild_id.member(ctx_discord, author.id).await {
                    Ok(member) => (member.display_name().into_owned(), member.face()),
                    Err(_) => (author.name.clone(), author.face()),
                }
            }
            serenity::Channel::Private(_) => (author.name.clone(), author.face()),
            _ => unreachable!(),
        };

        Ok(
            self.send(|b| {
                b.ephemeral(true);
                b.embed(|e| {
                    e.colour(RED);
                    e.title("An Error Occurred!");
                    e.description(format!("Sorry but {}, to fix this, please {}!", error, fix));
                    e.author(|a| {
                        a.name(name);
                        a.icon_url(avatar_url)
                    });
                    e.footer(|f| f.text(format!(
                        "Support Server: {}", self.data().config.main_server_invite
                    )))
                })
            })
            .await?
        )
    }
}

#[serenity::async_trait]
impl SerenityContextAdditions for serenity::Context {
    async fn user_from_dm(&self, dm_name: &str) -> Option<serenity::User> {
        lazy_static! {
            static ref ID_IN_BRACKETS_REGEX: Regex = Regex::new(r"\((\d+)\)").unwrap();
        }

        let re_match = ID_IN_BRACKETS_REGEX.captures(dm_name)?;
        let user_id: u64 = re_match.get(1)?.as_str().parse().ok()?;
        self.http.get_user(user_id).await.ok()
    }

    async fn join_vc(
        &self,
        lavalink: &LavalinkClient,
        guild_id: serenity::GuildId,
        channel_id: serenity::ChannelId,
    ) -> Result<(), Error> {
        let manager = songbird::get(self).await.unwrap();

        let (_, handler) = manager.join_gateway(guild_id.0, channel_id.0).await;
        Ok(lavalink.create_session_with_songbird(&handler?).await?)
    }
}

impl<T> OptionTryUnwrap<T> for Option<T> {
    #[track_caller]
    fn try_unwrap(self) -> Result<T, Error> {
        match self {
            Some(v) => Ok(v),
            None => Err(Error::None({
                let location = std::panic::Location::caller();
                format!("Unexpected None value on line {} in {}", location.line(), location.file())
            }))
        }
    }
}