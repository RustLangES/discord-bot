use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
    CreateInteractionResponse, CreateInteractionResponseMessage, EditInteractionResponse,
};

use songbird::input::{Compose, YoutubeDl};

use reqwest::Client as HttpClient;
use songbird::tracks::Track;
use songbird::Event;
use tracing::warn;

use crate::music::events::MusicEventEndHandler;
use crate::music::models::MusicPlaylistItem;
use crate::music::{self, MusicStore};

pub fn register() -> CreateCommand {
    CreateCommand::new("play")
        .description("Reproduce musica en un canal de voz. Puedes escribir una busqueda o una url")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "url_search",
                "URL del video de Youtube / Busqueda",
            )
            .kind(CommandOptionType::String)
            .required(true),
        )
}

pub async fn run(http_client: &HttpClient, ctx: &Context, cmd: &CommandInteraction) -> String {
    let Some(url) = cmd.data.options.iter().find(|opt| opt.name == "url_search") else {
        return String::from("Falta la URL o busqueda");
    };

    if let Err(err) = music::join(ctx, cmd).await {
        return err;
    };

    let url = url.value.as_str().unwrap().to_owned();

    let is_search = !url.starts_with("http");

    let guild_id = cmd.guild_id.unwrap();

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut src = if is_search {
            YoutubeDl::new_search(http_client.clone(), url.clone())
        } else {
            YoutubeDl::new(http_client.clone(), url.clone())
        };

        let data = CreateInteractionResponseMessage::new().content("Procesando cancion");
        let builder = CreateInteractionResponse::Defer(data);
        if let Err(why) = cmd.create_response(&ctx.http, builder).await {
            warn!("Cannot respond to slash command: {}", why);
        }

        let metadata = src.aux_metadata().await;

        let metadata = match metadata {
            Ok(m) => m,
            Err(err) => {
                let builder = EditInteractionResponse::new()
                    .content(format!("No se pudo obtener la cancion: {err}"));

                if let Err(why) = cmd.edit_response(&ctx.http, builder).await {
                    warn!("Cannot respond to slash command: {}", why);
                }

                return String::new();
            }
        };

        let song_name = metadata
            .title
            .clone()
            .unwrap_or(String::from("[object Object]"));

        let playlist_item = MusicPlaylistItem::from_metadata(metadata, src.into(), url);

        MusicStore::play_item(
            ctx.data.read().await.get::<MusicStore>().unwrap().clone(),
            handler_lock,
            playlist_item,
        );

        let builder =
            EditInteractionResponse::new().content(format!("Reproduciendo \"{song_name}\""));

        if let Err(why) = cmd.edit_response(&ctx.http, builder).await {
            warn!("Cannot respond to slash command: {}", why);
        }

        String::new()
    } else {
        String::from("El bot no se encuentra en un canal de voz")
    }
}