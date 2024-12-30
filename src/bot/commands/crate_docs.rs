use crate::bot;
use crate::bot::commands::krate::autocomplete;
use poise::serenity_prelude::CreateEmbed;
use poise::CreateReply;
use regex::Regex;
use reqwest::get;
use scraper::{Html, Selector};
use tracing::info;

fn strip_html(input: &str) -> String {
    Regex::new(r"</?[^>]+>")
        .unwrap()
        .replace_all(input, "")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .to_string()
}

async fn scrap_doc(
    ctx: bot::Context<'_>,
    search: &str,
    package: &str,
    url: &str,
) -> Result<Option<(String, String, String, String)>, bot::Error> {
    let response = get(url).await?;

    if response.status() != 200 {
        ctx.say(format!("The crate `{package}` was not found."))
            .await
            .ok();

        return Ok(None);
    }

    let html = response.text().await?;

    let selector = Selector::parse("ul.all-items > li > a").unwrap();
    let elements = Html::parse_document(&html);
    let elements = elements.select(&selector);

    let Some((_, url)) = elements
        .into_iter()
        .map(|elem| (elem.inner_html(), elem.attr("href").unwrap()))
        .find(|(name, _)| name.to_lowercase().contains(&search.to_lowercase()))
    else {
        ctx.say(format!(
            "The expression `{search}` was not found on `{package}`."
        ))
        .await
        .ok();

        return Ok(None);
    };

    let html = get(url).await?.text().await?;

    let html = Html::parse_document(&html);

    let element_type_selector = Selector::parse(".main-heading > h1").unwrap();
    let element_name_selector = Selector::parse(".main-heading > h1 > span").unwrap();
    let element_code_selector = Selector::parse(".item-decl > code").unwrap();
    let element_description_selector = Selector::parse(".docblock > p:last-child").unwrap();

    let element_type = html
        .select(&element_type_selector)
        .into_iter()
        .next()
        .unwrap()
        .inner_html();
    let element_type = element_type.split_once(" ").unwrap().0;
    let element_name = html
        .select(&element_name_selector)
        .into_iter()
        .next()
        .unwrap()
        .inner_html();
    let element_name = element_name.split(" ").last().unwrap();
    let element_code = strip_html(
        &html
            .select(&element_code_selector)
            .into_iter()
            .next()
            .unwrap()
            .inner_html(),
    );
    let element_description = html
        .select(&element_description_selector)
        .into_iter()
        .next()
        .unwrap()
        .inner_html()
        .replace("<code>", "`")
        .replace("</code>", "`");

    Ok(Some((
        element_type.into(),
        element_name.into(),
        element_code.into(),
        element_description.into(),
    )))
}

#[poise::command(slash_command, prefix_command, broadcast_typing, category = "Crates")]
pub async fn crate_docs(
    ctx: bot::Context<'_>,
    #[description = "El nombre del crate, crate@version | crate"]
    #[rename = "crate"]
    #[autocomplete = "autocomplete"]
    package: String,
    #[description = "Buscar un nombre o expresion minima, como Struct::method."] search: String,
) -> Result<(), bot::Error> {
    let (name, version) = package.split_once("@").unwrap_or((&package, "latest"));
    let url = format!("https://docs.rs/{name}/{version}/{name}/all.html");
    let Some((element_type, element_name, element_code, element_description)) =
        scrap_doc(ctx, &package, &search, &url).await?
    else {
        return Ok(());
    };

    ctx.send(
        CreateReply::default().embed(
            CreateEmbed::new()
                .title(format!("{element_type} {element_name}"))
                .description(format!(
                    "```rs\n{element_code}\n```\n{element_description}\n\nSee more: {url}"
                )),
        ),
    )
    .await?;

    Ok(())
}

/// Returns whether the given type name is the one of a primitive.
#[rustfmt::skip]
fn is_in_std(name: &str) -> bool {
	name.chars().next().is_some_and(char::is_uppercase)
		|| matches!(
            name,
            "f32" | "f64"
                | "i8" | "i16" | "i32" | "i64" | "i128" | "isize"
                | "u8" | "u16" | "u32" | "u64" | "u128" | "usize"
                | "char" | "str"
                | "pointer" | "reference" | "fn"
                | "bool" | "slice" | "tuple" | "unit" | "array"
        )
}

#[poise::command(broadcast_typing, slash_command, category = "Crates")]
pub async fn docs(
    ctx: bot::Context<'_>,
    #[description = "Buscar un nombre o expresion minima, como Struct::method."] query: String,
) -> Result<(), bot::Error> {
    let mut query_iter = query.splitn(2, "::");
    let first_path_element = query_iter.next().unwrap();
    let mut url = "https://doc.rust-lang.org/stable/std/".to_owned();

    if is_in_std(first_path_element) {
        url += "?search=";
        url += &query;
    } else if let Some(item_path) = query_iter.next() {
        url += "?search=";
        url += item_path;
    }

    let Some((element_type, element_name, element_code, element_description)) =
        scrap_doc(ctx, &query, "std docs", &url).await?
    else {
        return Ok(());
    };

    ctx.send(
        CreateReply::default().embed(
            CreateEmbed::new()
                .title(format!("{element_type} {element_name}"))
                .description(format!(
                    "```rs\n{element_code}\n```\n{element_description}\n\nSee more: {url}"
                )),
        ),
    )
    .await?;

    Ok(())
}
