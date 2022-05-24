use std::collections::HashMap;
use std::time::Duration;
use tokio::stream::{Stream, StreamExt};
use tokio::time::{timeout_at, Instant};
use url::Url;

use ya_client::web::WebClient;
use yarapi::rest::{self, Proposal};

use ya_agreement_utils::agreement::{expand, ProposalView};
use ya_client_model::market::NewDemand;
use ya_client_model::NodeId;

pub struct NodeInfo {
    pub id: NodeId,
    pub name: String,
}

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

async fn list_nodes(
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
        Err(e) => Err(e),
    }))
}

pub async fn collect_for(
    server_api_url: Url,
    appkey: &str,
    timeout: Duration,
) -> anyhow::Result<HashMap<NodeId, String>> {
    let mut collection = HashMap::new();
    let stop = Instant::now() + timeout;

    let mut stream = Box::pin(list_nodes(server_api_url, appkey, "hybrid").await?);

    while let Ok(Some(result)) = timeout_at(stop, stream.next()).await {
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
        let value = expand(serde_json::to_value(proposal.props())?);
        let view = ProposalView::try_from(value)?;

        Ok(NodeInfo {
            id: view.issuer,
            name: view.pointer_typed("/golem/node/id/name")?,
        })
    }
}
