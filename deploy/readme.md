## Preparation

Open [cluster.yaml](/cluster.yaml) in your favourite editor and update the settings, specially `hetzner_api` and `cluster_name`

## Deploy

If using an SSH key with password for your nodes, run the below before starting:

```bash
eval "$(ssh-agent -s)"
ssh-add --apple-use-keychain ~/.ssh/hetzner
```

Then you're ready to deploy.

```bash
hetzner-k3s create --config cluster.yaml
```

Because we are using a domain for the control plane `k3s.kubetailor.io`, we need to add the Master node IP in a DNS record, so after the first server is created, grab the Public IP and create an A record pointing to that IP.
Don't worry if `hetzner-k3s` fails before you added it. Just add it and restart `hetzner-k3s` command.

## Post-install

Now we need to install Calico in order to get networking up in the cluster.
Set the `kubeconfig` env var to the current path

```bash
export KUBECONFIG=$(pwd)/kubeconfig
kubectl create -f https://raw.githubusercontent.com/projectcalico/calico/v3.27.0/manifests/tigera-operator.yaml
kubectl create -f https://raw.githubusercontent.com/projectcalico/calico/v3.27.0/manifests/custom-resources.yaml
```

### Validation

```bash
kubectl get pods -A
kubectl get nodes
```

Nodes should be ready and pods Running.

### Kubetailor dependencies

Please install following in the correct order.

- [external-dns](https://github.com/kubernetes-sigs/external-dns/)

```bash
kubectl create ns external-dns
kubectl create secret generic cloudflare-creds -n external-dns --from-literal=token="<your-cloudflare-token>"
kubectl apply -f external-dns.yaml
```

- [cert-manager](https://github.com/cert-manager/cert-manager)

```bash
helm repo add jetstack https://charts.jetstack.io --force-update
helm repo update
helm install \
  cert-manager jetstack/cert-manager \
  --namespace cert-manager \
  --create-namespace \
  --version v1.14.2 \
  --set installCRDs=true
kubectl apply -f https://raw.githubusercontent.com/mpwsh/k3s-arm64/main/cert-manager/issuers/letsencrypt-prod-issuer.yaml
```

- [ingress-nginx](https://github.com/kubernetes/ingress-nginx)

```bash
helm repo add ingress-nginx https://kubernetes.github.io/ingress-nginx
helm repo update
helm upgrade --install nginx -n ingress-nginx --create-namespace \
ingress-nginx/ingress-nginx -f nginx.yaml
# Add an A Record pointing to your load balancer IP with name 'lb'
```

- [reloader](https://github.com/stakater/Reloader)

```bash
helm repo add stakater https://stakater.github.io/stakater-charts
helm repo update
helm upgrade --install reloader stakater/reloader -n reloader --create-namespace --set fullnameOverride=reloader
```

- [longhorn](https://github.com/longhorn/longhorn-engine)

```bash
# Prepare a basic auth user/pw for the UI
htpasswd -c auth your-username
kubectl create secret generic basic-auth --from-file=auth -n longhorn-system
helm repo add longhorn https://charts.longhorn.io
helm repo update
helm upgrade --install longhorn longhorn/longhorn -n longhorn-system --create-namespace -f longhorn.yaml --set fullnameOverride=longhorn
```

- [garage](https://garagehq.deuxfleurs.fr/)

```bash
helm repo add mpwsh https://charts.mpw.sh
helm repo update
helm upgrade --install --create-namespace --namespace garage garage mpwsh/garage -f garage/values.yaml

#Expose the ADMIN API endpoint
kubectl patch service garage -n garage -p '{"spec":{"ports":[{"port":9303,"targetPort":9303,"protocol":"TCP"}]}}'
# Port forward that port to your local
kubectl port-forward svc/garage 3903:3903
# Head over to garage folder
cd garage
#Set the admin_key you set up in garage chart values
export GARAGE_ADMIN_TOKEN="<your-key>"
export ADMIN_TOKEN_HEADER="Authorization: Bearer ${GARAGE_ADMIN_TOKEN}"
curl -sfL -H "${ADMIN_TOKEN_HEADER}" localhost:3903/v1/status | jq
#Take not of the node IDS.
#Open layout.json and update the node ids and capacity (in bytes)
#Send the layout
curl -X POST -sfL -H "${ADMIN_TOKEN_HEADER}" --data @layout.json localhost:3903/v1/layout | jq

# If need to revert layout changes use:
# curl -X POST -sfL -H "${ADMIN_TOKEN_HEADER}" localhost:3903/v1/layout/revert --data '{"version": 1}'
#If all went ok, apply the layout. Version might be wrong. make sure to update that
curl -X POST -sfL -H "${ADMIN_TOKEN_HEADER}" localhost:3903/v1/layout/apply --data '{"version": 1}' | jq
#Run ./create-key.sh
./create-key.sh
#open bucket.json and set the access key you got from the create_key call
#run create-bucket.sh
./create-bucket.sh
```

- [quickwit](https://github.com/quickwit-oss/quickwit)

```bash
helm repo add quickwit https://helm.quickwit.io
helm repo update quickwit
helm upgrade --install quickwit -n quickwit --create-namespace quickwit/quickwit -f quickwit.yaml
```

- [otel-collector](https://github.com/open-telemetry/opentelemetry-collector)

```bash
helm repo add open-telemetry https://open-telemetry.github.io/opentelemetry-helm-charts
helm repo update
helm upgrade --install otel-collector -n otel --create-namespace open-telemetry/opentelemetry-collector -f otel-collector.yaml
```

- [portier](https://github.com/portier/portier-broker)

```bash
#Not required for now. Use https://broker.mpw.sh
helm upgrade --install --create-namespace --namespace garage garage mpwsh/portier -f values.yaml
```

- [redis](https://github.com/redis/redis) - for the console session management

```bash
helm repo add bitnami https://charts.bitnami.com/bitnami
helm repo update
helm upgrade --install redis -n kubetailor --create-namespace bitnami/redis -f redis.yaml
```

- [kubetailor](https://github.com/mpwsh/kubetailor)

```bash
kubectl apply -f crd.yaml \
-f no-role-sa.yaml \
-f clusterrole.yaml
```

- apisix [not implemented]
- keda [not implemented]
- prometheus [not implemented]

REsearch:

- keel.sh [not implemented]
