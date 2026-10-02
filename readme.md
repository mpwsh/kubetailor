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
## Deploy throwaway cluster with k3d
## Install k3d
curl -s https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | bash
## Create the cluster
k3d cluster create kubetailor
## create the kubetailor namespace
kubectl create namespace kubetailor
## Deploy the crd, cluster role and service account for deployments
kubectl create -f deploy/crd.yaml -f deploy/clusterrole.yaml  -f deploy/no-role-sa.yaml
## Start the operator
## (At this point you can start deploying TailoredApp manifests if you want)
cargo run --bin operator
## Start the backend server -If trying out the backend API-
CONFIG_PATH=config/server/conf.yaml cargo run --bin server
```

Use the API to deploy an NGINX container with a static `index.html` file [basic.json](./examples/basic.json)

```bash
curl --request POST --url http://127.0.0.1:8080/ \
  --header 'Content-Type: application/json' --data "@examples/basic.json"
```

Or deploy a base NGINX container that syncs to a repo hosting your static site [git.json](./examples/git.json)

```bash
curl --request POST --url http://127.0.0.1:8080/ \
  --header 'Content-Type: application/json' --data "@examples/git.json"
```

You should be see a `Deployment`, `ConfigMap` and `Ingress` being created now.

You can hit your service using `port-forward`

```bash
kubectl port-forward svc/example -n kubetailor 5050:80
```

Now visit [localhost:5050](http://localhost:5050)

### Testing the console

The console needs a redis compatible kv store to keep track of sessions. In this case we'll use [keydb](https://keydb.dev)

```bash
## Start keydb
docker run --name kubetailor-keydb -p 6379:6379 eqalpha/keydb:latest
### Start the console
APP_ENVIRONMENT=local cargo run --bin console
```

Console Web UI should be available at [localhost:8080](http://localhost:8080)

## Regions and ports exposed at the node

A `TailoredApp` normally gets one HTTP port (`container.port`) published through the ingress
controller. Two optional fields cover everything else:

- `deployment.region` pins the app to a node labelled `topology.kubernetes.io/region=<region>`
  (every node provisioned by [flint](https://github.com/mpwsh/flint) carries it). With one node
  per region that *is* the node, and the operator points the app's DNS record at that node's
  public IP rather than a shared load balancer, so traffic terminates in the region the user asked
  for.
- `container.ports` lists extra ports, each with a `protocol` (`TCP`/`UDP`) and an `expose` mode:

  | `expose`   | What you get                                                                                   |
  |------------|------------------------------------------------------------------------------------------------|
  | `cluster`  | A port on the app's Service, reachable inside the cluster only (default).                      |
  | `node`     | `hostPort`: the same port number on the node running the pod. `<node ip>:<port>`, nothing in between. |
  | `nodePort` | A second Service of type `NodePort` with `externalTrafficPolicy: Local`; Kubernetes picks the port. |

  For `node` and `nodePort` the NetworkPolicy admits the internet on exactly that port and
  protocol; everything else stays closed. The cloud firewall is opened through the flint
  controller (below); without it, `flint cluster firewall <cluster> --allow udp:7777` by hand.
- `container.resources` (`cpu`, `memory`, Kubernetes quantities) is what one replica needs. It
  becomes the pod's requests and memory limit — and it is what lets the scheduler say a node is
  full, which is how the cluster knows to grow (below). Apps without it pack onto any node.

`ingress` is optional. An app with only node-exposed ports needs none; one with `ingress.domains`
but no `container.port` gets no Ingress object either, the domains then just name the app and the
operator writes the external-dns `hostname`/`target` annotations on its Service.

Once the pods are scheduled the operator fills `status`:

```yaml
status:
  nodes:
    - name: kt-scl-0001
      ip: 45.77.0.10
      region: scl
  endpoints:
    - ip: 45.77.0.10
      port: 7777
      protocol: UDP
```

`kubectl get tapp` shows the region and node IP as columns; `status.message` says why there is no
placement yet (`no node in region scl`). The node's public IP is read from the
`flint.mpw.sh/public-ip` label (override with `KUBETAILOR_PUBLIC_IP_LABEL`), falling back to the
node's `ExternalIP`, then `InternalIP`.

Examples: [udp-echo.yaml](./examples/udp-echo.yaml) (UDP only, no ingress),
[game-server.yaml](./examples/game-server.yaml) (web page through the ingress, game port at the
node), [udp-echo.json](./examples/udp-echo.json) (the same through the server API; the server's
`nodePortRange` config bounds the ports users may expose, default `1024-29999`).

### Nodes on demand (flint controller)

With the [flint controller](https://github.com/mpwsh/flint#controller-mode) in the cluster, the
operator asks for infrastructure with its objects instead of expecting an admin to run `flint`:

- **A region without a node.** An app pinned to `waw` when no node carries that label gets a
  `NodeClaim` named `region-waw`; flint adds the node, the scheduler places the pod. Meanwhile
  `status.message` reports the claim's progress (`node kt-waw-a1b2 is Provisioning: …`) or its
  refusal (`claim region-waw is Failed: denied: region waw is not allowed`).
- **A region that is full.** A pod the scheduler has refused for more than 30 s because of
  `Insufficient cpu`/`memory` or `didn't have free ports` (two copies of a `node`-exposed port
  cannot share a node) gets one more claim for its region, `region-waw-2`, and so on. Never
  more than one claim at a time per region: while one is Pending or Provisioning, the app waits
  on it. Apps that name no region are grown in `KUBETAILOR_DEFAULT_REGION`, else the control
  plane's region. The flint controller's policy (`maxNodesPerRegion`, `allowedRegions`, …) is
  the budget; a refusal shows up in the app's status rather than as a silent Pending pod.
- **Ports.** Every `node`/`nodePort` port becomes a `FirewallRule` (`<app>-udp-27015`) owned by
  the app, so the cloud firewall opens and closes with it. `nodePort` rules carry the port
  Kubernetes allocated.
- **Giving nodes back.** Every 5 minutes the operator looks at the claims it made. A node that
  has run no app pods for `KUBETAILOR_NODE_IDLE_MINUTES` (default 60, counted from when it was
  first seen empty, so from Ready for a node nothing ever landed on) has its claim deleted and
  flint drains and destroys it. A claim that never got a node is deleted once no app asks for
  its region any more. Claims made by hand (no `kubetailor.io/managed` label) are never touched.

The ClusterRole in [deploy/clusterrole.yaml](./deploy/clusterrole.yaml) covers the two kinds. On
a cluster without the flint CRDs the operator says so once at start-up and does none of this;
apps still deploy onto the nodes there are.

### Live updates

Editing a `TailoredApp` (`kubectl apply`/`edit`, or the server's `PUT`) re-renders its resources
at once. The operator is level-triggered: on every reconcile it server-side applies each child
resource under the `kubetailor` field manager, so

- fields the spec stopped setting are removed, resources the spec stopped asking for are pruned
  (the env ConfigMap, the Ingress, the `nodePort` Service, ...);
- what other controllers own is left alone: the Deployment controller's revision annotation, a
  `kubectl rollout restart` stamp, allocated `clusterIP`s and node ports;
- an unchanged spec is a no-op on the API server, so the periodic resync (every 60s) is cheap and
  cannot feed back into itself.

PersistentVolumeClaims are the exception: a volume removed from the spec keeps its PVC and data
until the app is deleted. PVCs and file ConfigMaps are named after a hash of their mount path
(`pvc-<app>-<id>`, `files-<app>-<id>`) rather than their position in the list, so adding one never
renames the others.

`status.observedGeneration` equal to `metadata.generation` means the resources reflect the spec as
it is now; `status.message` carries the reason when an apply fails (for example an ingress with
domains but no `container.port`).

## Console conventions (htmx 4)

The console is server-rendered HTML with [htmx 4](https://four.htmx.org) for navigation and
partial updates, Alpine only for purely visual state (open menus, wizard steps, repeater rows).
Scripts are pinned with subresource integrity in `web/templates/head/scripts.hbs`; the
`hx-alpine-compat` extension keeps Alpine state through morph swaps and `hx-preload` fetches
sidebar pages on hover. Rules the handlers follow:

- Every page renders inside `#content`. A handler checks `req.is_htmx()` to render the full
  shell or just the page (`initial`), and `req.targets("deployments-table")` to render only the
  part that asked, so a poller never gets more HTML than it swaps. `is_htmx()` is false for a
  back/forward restore (`HX-Request-Type: full`): htmx 4 re-fetches the URL and picks
  `[hx-history-elt]` out of a whole page. `HX-Target` arrives as `tag#id`.
- Attribute inheritance is explicit: the sidebar's `hx-target:inherited` etc. cover its links;
  everything else carries its own attributes.
- Pollers whose markup keeps the same attributes morph over themselves (`hx-swap="outerMorph"`:
  the deployments table, the log pane), so open menus, focus and scroll survive a refresh.
  Pollers the server stops by dropping `hx-trigger` (deploy and delete progress) must use
  `outerHTML`: a morph keeps the element and its trigger alive.
- Pausing a poller is a checkbox the trigger reads on every tick
  (`hx-trigger="every[document.getElementById('autorefresh')?.checked] 5s"`), never an attribute
  rewritten by JavaScript: htmx reads `hx-*` once, when it processes the element.
- Redirects go through `utils::redirect` (`HX-Location` into `#content` for htmx callers, a 303
  otherwise) or `redirect_full` (`HX-Redirect`, for leaving the shell). A plain 303 is followed by
  the fetch and the caller receives the target's fragment instead of navigating.
- Forms post as forms. Validation problems come back as a `422` whose body the form routes into
  `#form-errors` (`hx-status:422="target:#form-errors"`), so the message lands above the form
  and the form keeps its state; success is a redirect. Other 4xx/5xx never swap (`noSwap` in the
  htmx config): their bodies are plain text.
- The wizard ends in a review: its last step posts the form to `/deployments/review`, which folds
  and validates it like a deploy, asks the API for the manifest it would create (`POST /preview`)
  and answers the review fragment with `HX-Trigger: review-ready`; Alpine then shows the review
  instead of the wizard. The form never leaves the DOM, so "Back to wizard" is a flag flip and
  the review's Deploy button submits it with `form="editForm"`.
- No `.unwrap()` on upstream calls: a backend hiccup is a warning in the row (`health: null`) or
  an inline error, not a 500 page.
- Repeating inputs (environment, ports) post flat, repeated fields; `form::tapp_from_form` folds
  them back in document order. The ports repeater is Alpine state seeded from the deployment with
  the `json` helper (`x-data="{ ports: {{{json tapp.container.ports}}} }"`): every row posts a
  `port_number` / `port_protocol` / `port_expose` triple, and a row with no number is ignored.
  The HTTP port is optional in the wizard; an app without one and without other ports is
  rejected before it reaches the API.

## Services

- [Operator](./crates/operator) - Listens for new `TailoredApps` and constructs and deploys native Kubernetes resources from there.
- [Server](./crates/server) - Receives simplified `TailoredApps` via HTTP and merges them with hard-coded values from its [config](./config/server/conf.yaml)
- [Console](./crates/console) - A simple reference console to build the JSON request to send to the server.

## Kubernetes Dependencies

- An ingress controller: [Traefik](https://github.com/traefik/traefik) (what [flint](https://github.com/mpwsh/flint) installs) or [ingress-nginx](https://github.com/kubernetes/ingress-nginx)
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

Console Web UI uses [PenguinUI](https://www.penguinui.com/) for the components and theme.
