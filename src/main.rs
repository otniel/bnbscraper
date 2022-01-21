use std::collections::HashSet;

use color_eyre::Report;
use reqwest::Client;
use select::document::Document;
use select::node::Node;
use select::predicate::{Class, Name, Predicate};
use tracing::info;
use tracing_subscriber::EnvFilter;

const ROOT_URL: &str = "https://www.bathandbodyworks.mx";

#[derive(Debug, Default)]
struct BnBItem {
    name: String,
    item_type: String,
    link: String,
    price: f32,
    discount: String,
}

#[tokio::main]
async fn main() -> Result<(), Report> {
    setup()?;

    info!("Starting Bath And Body Works scraper...");

    let client = Client::new();
    let res = client.get(ROOT_URL).send().await?.text().await?;

    let document = Document::from(res.as_str());
    let links = document.find(Name("a"));

    let uniq_links: Vec<String> = links
        .into_iter()
        .map(|node| node.attr("href").unwrap())
        .collect::<HashSet<_>>()
        .into_iter()
        .filter(|link| !link.contains("www"))
        .map(|link| format!("{}{}", ROOT_URL, link))
        .collect();
    info!("Landing page links fetched...");

    let mut all_items: Vec<BnBItem> = Vec::new();

    for link in uniq_links {
        info!("Procesing link {}", &link);

        let res = reqwest::get(link).await?.text().await?;
        let document = Document::from(res.as_str());
        let products = document.find(Class("product-item"));

        for product in products {
            let mut bnb_item = BnBItem::default();
            process_item(
                product,
                Class("product-item__caption"),
                Name("a"),
                |caption: Node| {
                    bnb_item.name = caption.text();
                    bnb_item.link = caption.attr("href").unwrap().to_owned();
                },
            );
            process_item(
                product,
                Class("product-item__form"),
                Name("li"),
                |item_type: Node| {
                    bnb_item.item_type = item_type.text();
                },
            );
            process_item(
                product,
                Class("product-item__price"),
                Name("span"),
                |price: Node| {
                    let price = price.text().replace("$", "");
                    let parsed_price = price.parse::<f32>();
                    if let Ok(parsed_price) = parsed_price {
                        bnb_item.price = parsed_price;
                    }
                },
            );
            process_item(
                product,
                Class("product-item__flags--discounts"),
                Name("p"),
                |discount: Node| {
                    bnb_item.discount = discount.text();
                },
            );

            all_items.push(bnb_item);
        }
    }

    info!("Finished!");
    info!("Total items: {}", all_items.len());

    Ok(())
}

fn process_item(
    item: Node,
    class: Class<&str>,
    descendant: Name<&str>,
    mut handler: impl FnMut(Node),
) {
    let link_node = item.find(class.descendant(descendant)).next();

    if let Some(unwrapped_node) = link_node {
        handler(unwrapped_node);
    };
}

fn setup() -> Result<(), Report> {
    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        std::env::set_var("RUST_LIB_BACKTRACE", "1")
    }
    color_eyre::install()?;

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info")
    }
    tracing_subscriber::fmt::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    Ok(())
}
