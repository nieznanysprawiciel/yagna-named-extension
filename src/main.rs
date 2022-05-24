mod cache;
mod list;
pub mod output;
pub mod yagna;

use crate::cache::Cache;
use crate::list::collect_for;
use crate::yagna::YagnaCommand;

use anyhow::{anyhow, bail};
use clap::{Parser, Subcommand};
use serde_json::Value;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use url::Url;

use crate::output::{CommandOutput, ResponseTable};
use ya_client_model::NodeId;

#[derive(Subcommand)]
enum Commands {
    Collect,
    #[clap(external_subcommand)]
    #[clap(setting = clap::AppSettings::Hidden)]
    DecorateCmd(Vec<String>),
}

#[derive(clap::Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(long, env = "YAGNA_APPKEY")]
    pub appkey: String,
    #[clap(long, env = "YAGNA_API_URL")]
    pub server_api_url: String,
    #[clap(long, env = "YAGNA_DATA_DIR")]
    pub datadir: PathBuf,
    #[clap(long, env = "YAGNA_JSON_OUTPUT")]
    pub json: bool,
    #[clap(subcommand)]
    pub command: Commands,
    #[clap(skip = "yagna-named.cache")]
    pub cache_file: String,
}

#[actix_rt::main]
pub async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    let args = Args::parse();

    env_logger::builder()
        .filter_module("yarapi::drop", log::LevelFilter::Off)
        .filter_module("ya_service_bus::connection", log::LevelFilter::Off)
        .filter_module("ya_service_bus::remote_router", log::LevelFilter::Off)
        .init();

    match &args.command {
        Commands::Collect => collector(&args).await,
        Commands::DecorateCmd(cmd) => decorate_command(&args, cmd).await,
    }
}

async fn collector(args: &Args) -> anyhow::Result<()> {
    let server_api_url: Url = args.server_api_url.parse()?;
    let cache_file = args.datadir.join(&args.cache_file);
    let mut cache = Cache::new(&cache_file).await?;

    loop {
        let nodes = collect_for(
            server_api_url.clone(),
            &args.appkey,
            Duration::from_secs(30),
        )
        .await?;

        cache.update_cache(nodes).await;
    }
}

async fn decorate_command(args: &Args, cmd: &Vec<String>) -> anyhow::Result<()> {
    let command = YagnaCommand::new()?
        .args(cmd)
        .args(&vec!["--json".to_string()]);

    let cache_file = args.datadir.join(&args.cache_file);
    let cache = Cache::new(&cache_file).await?;

    let result = command
        .run()
        .await
        .map_err(|e| anyhow!("Error executing yagna: {}", e))?;
    let json = decorate_table(result, &cache)
        .map_err(|e| anyhow!("Error appending nodes names. {}", e))?;

    log::trace!("{:?}", json);

    CommandOutput::from(ResponseTable::from_json(json)?).print(args.json);
    Ok(())
}

fn decorate_table(mut json: Value, cache: &Cache) -> anyhow::Result<Value> {
    let rows = match json.as_array_mut() {
        None => bail!("Expected an array"),
        Some(array) => array,
    };

    for row in rows {
        process_row2(row, cache).ok();
    }

    Ok(json)
}

fn process_row2(row: &mut Value, cache: &Cache) -> anyhow::Result<()> {
    let row = match row.as_object_mut() {
        None => bail!("Expected object."),
        Some(row) => row,
    };

    let name = match row
        .get("nodeId")
        .map(|id| id.as_str().map(|id| NodeId::from_str(id)))
    {
        Some(Some(Ok(node_id))) => match cache.node_name(node_id) {
            Some(name) => name,
            None => "-".to_string(),
        },
        None => {
            log::warn!("Row doesn't contain `nodeId` field.");
            "-".to_string()
        }
        _ => {
            log::warn!("Failure parsing `nodeId` field.");
            "-".to_string()
        }
    };

    row.insert("name".to_string(), Value::String(name));
    Ok(())
}
