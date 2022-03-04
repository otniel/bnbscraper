use std::collections::{HashMap, HashSet};
use std::fs::File;

use color_eyre::Report;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use reqwest::Client;
use select::document::{Document, Find};
use select::node::Node;
use select::predicate::{Class, Name, Predicate};
use serde::{Deserialize, Serialize};
use tracing::info;
use tracing_subscriber::EnvFilter;

const ROOT_URL: &str = "https://www.bathandbodyworks.mx";

#[derive(Serialize, Deserialize, Debug, Default)]
struct BnBItem {
    name: String,
    item_type: String,
    link: String,
    price: f32,
    price_promo: f32,
    discount: String,
}

impl PartialEq for BnBItem {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.item_type == other.item_type
    }
}

#[tokio::main]
async fn main() -> Result<(), Report> {
    setup()?;

    info!("Starting Bath And Body Works scraper...");

    let client = Client::new();
    let res = client.get(ROOT_URL).send().await?.text().await?;

    let document = Document::from(res.as_str());
    let links = document.find(Name("a"));
    let uniq_links: Vec<String> = get_unique_links(links);

    info!("Landing page links fetched...");

    let mut all_items: Vec<BnBItem> = Vec::new();
    let mut items_futures = uniq_links
        .iter()
        .map(|link| process_link(link))
        .collect::<FuturesUnordered<_>>();

    while let Some(result) = items_futures.next().await {
        if let Ok(products) = result {
            for product in products {
                if !all_items.contains(&product) {
                    all_items.push(product);
                }
            }
        }
    }

    info!("Finished!");
    info!("Total items: {}", all_items.len());

    let mut grouped: HashMap<&str, Vec<&BnBItem>> = HashMap::new();
    for item in all_items.iter() {
        grouped
            .entry(item.discount.as_str())
            .or_insert_with(Vec::new)
            .push(item);
    }

    let json_file = "/Users/otniel/Documents/code/rust/bnbscraper/data.json";
    serde_json::to_writer(&File::create(json_file)?, &grouped)?;

    Ok(())
}

async fn process_link(link: &str) -> Result<Vec<BnBItem>, Report> {
    info!("Processing link: {}", link);
    let res = reqwest::get(link).await?.text().await?;
    let document = Document::from(res.as_str());
    let products = document.find(Class("product-item"));

    let mut products_in_link = vec![];
    for product in products {
        let mut bnb_item = BnBItem::default();
        process_product(product, &mut bnb_item);

        if !products_in_link.contains(&bnb_item) {
            products_in_link.push(bnb_item);
        }
    }
    Ok(products_in_link)
}

fn process_product(product: Node, mut bnb_item: &mut BnBItem) {
    extract_name_and_link(product, &mut bnb_item);
    extract_item_type(product, &mut bnb_item);
    extract_price(product, &mut bnb_item);
    extract_price_promo(product, &mut bnb_item);
    extract_discount(product, &mut bnb_item);
}

fn get_unique_links(links: Find<Name<&str>>) -> Vec<String> {
    links
        .into_iter()
        .map(|node| node.attr("href").unwrap())
        .collect::<HashSet<_>>()
        .into_iter()
        .filter(|link| !link.contains("www"))
        .map(|link| format!("{}{}", ROOT_URL, link))
        .collect()
}

fn extract_discount(product: Node, bnb_item: &mut BnBItem) {
    process_attribute(
        product,
        Class("product-item__flags--discounts"),
        Name("p"),
        |discount: Node| {
            bnb_item.discount = discount.text();
        },
    );
}

fn extract_price(product: Node, bnb_item: &mut BnBItem) {
    process_attribute(
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
}

fn extract_price_promo(product: Node, bnb_item: &mut BnBItem) {
    process_attribute(
        product,
        Class("product-item__price"),
        Class("price-new"),
        |price: Node| {
            let price = price.text().replace("$", "");
            let parsed_price = price.parse::<f32>();
            if let Ok(parsed_price) = parsed_price {
                bnb_item.price_promo = parsed_price;
            }
        },
    );
}
fn extract_item_type(product: Node, bnb_item: &mut BnBItem) {
    process_attribute(
        product,
        Class("product-item__form"),
        Name("li"),
        |item_type: Node| {
            bnb_item.item_type = item_type.text();
        },
    );
}

fn extract_name_and_link(product: Node, bnb_item: &mut BnBItem) {
    process_attribute(
        product,
        Class("product-item__caption"),
        Name("a"),
        |caption: Node| {
            bnb_item.name = caption.text();
            bnb_item.link = caption.attr("href").unwrap().to_owned();
        },
    );
}

fn process_attribute<T>(item: Node, class: Class<&str>, predicate: T, mut handler: impl FnMut(Node))
where
    T: Predicate,
{
    let link_node = item.find(class.descendant(predicate)).next();

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
