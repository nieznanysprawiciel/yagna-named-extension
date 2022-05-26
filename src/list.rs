use anyhow::{anyhow, bail};
use futures::{FutureExt, StreamExt};
use std::collections::HashMap;
use std::pin::Pin;
use std::time::Duration;
use tokio::stream::{Stream, StreamMap};
use tokio::time::{timeout_at, Instant};
use url::Url;

use ya_client::web::WebClient;
use yarapi::rest::{self, Proposal};

use ya_client_model::market::NewDemand;
use ya_client_model::NodeId;

pub struct NodeInfo {
    pub id: NodeId,
    pub name: String,
}

pub type ListingStream = Pin<Box<dyn Stream<Item = anyhow::Result<NodeInfo>>>>;

pub fn create_demand(subnet: &str) -> NewDemand {
    log::info!("Using subnet: {}", subnet);

    let properties = serde_json::json!({
        "golem.node.id.name": "Named node scanner",
        "golem.node.debug.subnet": subnet,
        "golem.srv.comp.expiration": chrono::Utc::now().timestamp_millis(),
    });

    let constraints = "()".to_string();

    NewDemand {
        properties,
        constraints,
    }
}

async fn list_nodes_stream(
    _server_api_url: Url,
    appkey: &str,
    subnet: &str,
) -> anyhow::Result<impl Stream<Item = anyhow::Result<NodeInfo>>> {
    let client = WebClient::with_token(appkey);
    let session = rest::Session::with_client(client.clone());
    let market = session.market()?;

    let demand = create_demand(&subnet);

    let subscription = market.subscribe_demand(demand.clone()).await?;
    log::info!("Created subscription [{}]", subscription.id().as_ref());

    Ok(subscription.proposals().map(|result| match result {
        Ok(proposal) => NodeInfo::try_from(proposal),
        Err(e) => bail!("Query Events result: {}", e),
    }))
}

async fn listing_streams(server_api_url: Url, appkey: &str) -> anyhow::Result<Vec<ListingStream>> {
    let subnets = vec!["hybrid", "devnet-beta", "public-beta"];
    let futures = subnets
        .iter()
        .map(|subnet| {
            list_nodes_stream(server_api_url.clone(), appkey, subnet).map(
                move |result| match result {
                    Ok(stream) => Ok(stream.boxed_local()),
                    Err(e) => {
                        log::warn!(
                            "Failed to create stream for subnet: {}. Error: {}. Ignoring",
                            subnet,
                            e
                        );
                        Err(e)
                    }
                },
            )
        })
        .collect::<Vec<_>>();

    Ok(futures::future::join_all(futures)
        .await
        .into_iter()
        .filter_map(|result| result.ok())
        .collect::<Vec<_>>())
}

pub async fn collect_for(
    server_api_url: Url,
    appkey: &str,
    timeout: Duration,
) -> anyhow::Result<HashMap<NodeId, String>> {
    let mut collection = HashMap::new();
    let stop = Instant::now() + timeout;

    let streams = listing_streams(server_api_url, appkey).await?;
    let mut streams_map = StreamMap::new();

    streams.into_iter().enumerate().for_each(|(i, stream)| {
        streams_map.insert(i.to_string(), stream);
    });

    while let Ok(Some((_, result))) = timeout_at(stop, streams_map.next()).await {
        match result {
            Ok(node) => {
                collection.insert(node.id, node.name);
            }
            Err(e) => log::warn!("Proposal failed: {}", e),
        }
    }

    Ok(collection)
}

impl TryFrom<Proposal> for NodeInfo {
    type Error = anyhow::Error;

    fn try_from(proposal: Proposal) -> Result<Self, Self::Error> {
        log::trace!("Parsing proposal [{}]. {}", proposal.id(), proposal.props());

        Ok(NodeInfo {
            id: proposal.issuer_id(),
            name: proposal
                .props()
                .pointer("/golem.node.id.name")
                .ok_or(anyhow!("No key `golem.node.id.name`"))?
                .as_str()
                .ok_or(anyhow!("Node name not found"))?
                .to_string(),
        })
    }
}
