use env_logger::Builder;
use futures::StreamExt;
use log::LevelFilter;
use prelude::*;

mod actions;
mod apply;
mod configmap;
mod context;
mod deployment;
mod error;
mod finalizer;
mod ingress;
mod netpol;
mod nodes;
mod placement;
pub mod prelude;
mod pvc;
mod reconciler;
mod secret;
mod service;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client: Client = Client::try_default()
        .await
        .expect("Expected a valid KUBECONFIG environment variable.");

    Builder::new().filter(None, LevelFilter::Info).init();

    // With the flint controller installed, apps can ask for nodes and open ports; without it
    // they still deploy, onto the nodes there are.
    let flint = nodes::available(&client).await;
    if flint {
        info!(
            "flint.mpw.sh found: nodes are requested through NodeClaims, idle ones released \
             after {} min",
            nodes::idle_window().as_secs() / 60
        );
    } else {
        warn!("flint.mpw.sh not found: apps cannot add nodes or open firewall ports");
    }
    let context: Arc<ContextData> = Arc::new(ContextData::new(client.clone(), flint));

    let (mut reload_tx, _reload_rx) = futures::channel::mpsc::channel(0);

    std::thread::spawn(move || {
        for _ in std::io::BufReader::new(std::io::stdin()).lines() {
            let _ = reload_tx.try_send(());
        }
    });
    let tapp = Api::<TailoredApp>::all(client.clone());
    if let Err(e) = tapp.list(&ListParams::default().limit(1)).await {
        error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        std::process::exit(1);
    }
    // Children emit bursts of events (a rollout is a dozen Deployment status updates); coalesce
    // them so one reconcile handles each burst.
    let controller = Controller::new(tapp, Config::default().any_semantic())
        .with_config(
            kubetailor::kube::runtime::controller::Config::default()
                .debounce(Duration::from_secs(2)),
        )
        .shutdown_on_signal();
    let controller = if flint {
        // A claim changing (node ready, refused) concerns the apps pinned to its region and any
        // app still waiting for something.
        let store = controller.store();
        let reaper_client = client.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(nodes::reap_interval()).await;
                if let Err(e) = nodes::reap_idle(&reaper_client, nodes::idle_window()).await {
                    error!("idle node check failed: {e}");
                }
            }
        });
        controller
            .owns(
                Api::<FirewallRule>::all(client.clone()),
                watcher::Config::default(),
            )
            .watches(
                Api::<NodeClaim>::all(client.clone()),
                watcher::Config::default(),
                move |claim| {
                    let region = claim.spec.region.clone();
                    store
                        .state()
                        .into_iter()
                        .filter(|app| {
                            app.spec.deployment.region.as_deref() == Some(region.as_str())
                                || app.status.as_ref().is_some_and(|s| s.message.is_some())
                        })
                        .map(|app| ObjectRef::from_obj(app.as_ref()))
                        .collect::<Vec<_>>()
                },
            )
    } else {
        controller
    };
    controller
        .owns(
            Api::<ConfigMap>::all(client.clone()),
            watcher::Config::default(),
        )
        .owns(
            Api::<Deployment>::all(client.clone()),
            watcher::Config::default(),
        )
        .owns(
            Api::<Service>::all(client.clone()),
            watcher::Config::default(),
        )
        .owns(
            Api::<Ingress>::all(client.clone()),
            watcher::Config::default(),
        )
        .owns(
            Api::<Secret>::all(client.clone()),
            watcher::Config::default(),
        )
        .owns(
            Api::<PersistentVolumeClaim>::all(client.clone()),
            watcher::Config::default(),
        )
        .owns(
            Api::<NetworkPolicy>::all(client.clone()),
            watcher::Config::default(),
        )
        .run(reconcile, on_error, context)
        .for_each(|reconciliation_result| async move {
            match reconciliation_result {
                Ok(resource) => {
                    info!("Reconciliation successful. Resource: {resource:?}");
                }
                Err(reconciliation_err) => {
                    error!("Reconciliation error: {reconciliation_err:?}")
                }
            }
        })
        .await;

    Ok(())
}
