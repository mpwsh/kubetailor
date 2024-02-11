**WARNING:** This is a work in progress.

Deploying random containers in your infrastructure is not very smart without taking the proper precautions.
Documentation is very poor, sorry. Feel free to open an issue if you get stuck or have feature proposals.

## Description

Kubetailor is a Kubernetes operator that simplifies the deployment of applications with their own domain, SSL certs, volumes, environment variables (via configMaps), secrets, volumes and fileMounts.
What makes this useful is the addition of a backend server that can receive simplified versions of a [TailoredApp manifest](./example-tapp.yaml) through its API and will merge all missing details using pre-filled information (annotations, storage classes, load balancer endpoint, etc) [from its configuration](./config/server/conf.yaml).
Kinda like how Helm merges `--set` arguments with values from a `values.yaml` file and the default values from the original chart.

Idea being:
You configure most of the `TailoredApp` beforehand and let your end-users provide few values that will spin a container for them.

### Deploy TL;DR

If you don't care about DNS and SSL and just want to see stuff being deployed, follow the steps below:

```bash
## Set your KUBECONFIG env var accordingly
export KUBECONFIG=~/.kube/your-config
## Deploy the crd and cluster role
kubectl create -f deploy/crd.yaml -f deploy/clusterrole.yaml
## Start the operator
## (At this point you can start deploying TailoredApp manifests if you want)
cargo run --bin operator
## Start the backend server -If trying out the backend API-
CONFIG_PATH=config/server/conf.yaml cargo run --bin server
```

Use the API to deploy an NGINX container with a static `index.html` file [basic.json](./examples/basic.json)

```bash
curl --request POST --url http://127.0.0.1:8080/ \
  --header 'Content-Type: application/json' --data "@tapp-example.json"
```

Or deploy a base NGINX container that syncs to a repo hosting your static site [git.json](./examples/git.json)

```bash
curl --request POST --url http://127.0.0.1:8080/ \
  --header 'Content-Type: application/json' --data "@tapp-git.json"
```

You should be see a `Deployment`, `ConfigMap` and `Ingress` being created now.

## Services

- [Operator](./crates/operator) - Listens for new `TailoredApps` and constructs and deploys native Kubernetes resources from there.
- [Server](./crates/server) - Receives simplified `TailoredApps` via HTTP and merges them with hard-coded values from its [config](./config/server/conf.yaml)
- [Console](./crates/console) - A simple reference console to build the JSON request to send to the server.

## Kubernetes Dependencies

- [NGINX Ingress](https://github.com/nginxinc/kubernetes-ingress)
- [External DNS](https://github.com/external-secrets/external-secrets)
- [Cert Manager](https://github.com/cert-manager/cert-manager)

### Optional

- [Reloader](https://github.com/stakater/Reloader)
- [Longhorn Engine](https://github.com/longhorn/longhorn-engine)
- [Portier broker](https://github.com/portier/portier-broker) (only if using [console](./crates/console))

### External dependencies

- DNS Provider (Supported by [external-dns](https://github.com/kubernetes-sigs/external-dns/#status-of-providers))
- CSI Provider (If not using [Longhorn Engine](https://github.com/longhorn/longhorn-engine))

> I should probably work on a helm chart for this. Manual install is the only way for now, sorry.
