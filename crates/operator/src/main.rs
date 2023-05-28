use env_logger::Builder;
use futures::StreamExt;
use log::LevelFilter;
use prelude::*;

mod actions;
mod configmap;
mod context;
mod deployment;
mod error;
mod finalizer;
mod ingress;
mod netpol;
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

    let context: Arc<ContextData> = Arc::new(ContextData::new(client.clone()));

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
    Controller::new(tapp, Config::default().any_semantic())
        .shutdown_on_signal()
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
        .run(reconcile, on_error, context)
        .for_each(|reconciliation_result| async move {
            match reconciliation_result {
                Ok(resource) => {
                    info!("Reconciliation successful. Resource: {resource:?}");
                },
                Err(reconciliation_err) => {
                    error!("Reconciliation error: {reconciliation_err:?}")
                },
            }
        })
        .await;

    Ok(())
}
