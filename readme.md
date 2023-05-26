**WARNING:** This is a work in progress.
Deployments are still not fully secured. Deploying random containers in your infrastructure is not very smart without taking the proper precautions.

Also:
Documentation is very poor and you need to build your own images.
Feel free to open an issue if you get stuck or have feature proposals.

### Description

Kubetailor is a Kubernetes operator that simplifies the deployment of applications with their own domain + certs, volumes, environment variables and secrets.
What makes this useful is the addition of a simple API that can receive simplified versions of `TailoredApp` manifests that will get merged with some hard-coded defaults you can set-up in your configuration.

Idea being: You configure most of the `TailoredApp` beforehand and let your end-users provide few values that will spin a container for them.

A reference front-end console implementation can be found in the [console](./crates/console) folder inside crates, the [backend server](./crates/server) can be found in [crates](./crates) too.

This guide provides a walkthrough on how to write your own manifest for the Kubetailor operator.

### Dependencies

- NGINX Ingress
- External DNS
- Cert Manager

## Optional deps

- Kyverno
- Cilium
- Reloader
- Longhorn Engine (To provision volumes using the nodes disk space instead of requesting volumes from your provider)
- Portier broker (if using [console](./console)

### Infrastructure dependencies

- DNS Provider (Could be any of the supported by [external-dns](https://github.com/kubernetes-sigs/external-dns/)
- CSI Provider
- A load balancer

### Writing your Manifest

Each application deployed through the Kubetailor operator is defined through a Custom Resource of kind `TailoredApp`.
The spec of this resource consists of several properties that provide the basic customization required for an application deployment.

Below is a general breakdown of the sections in a `TailoredApp` spec:

### Labels

```yaml
labels:
  owner: an-owner
  tapp: example-tapp
```

### Deployment

This includes the container image, port, number of replicas, build and run commands, volumes, and file mounts.

```yaml
deployment:
  container:
    image: nginx:latest
    port: 80
    replicas: 2
    volumes:
      # PathtoMount: Size
      /home/test/123: 40Mi
      /home/test/anotherone: 100Mi
    fileMounts:
      # PathToMount: Content
      /etc/config.toml: |-
        [someconfig]
        avalue= true
```

### Ingress

In the ingress section you must provide some annotations to `external-dns` in trigger the creation of required `CNAME` entries. Same goes for `cert-manager` annotations.
Basically this just the default ingress Class but really simplified, so it only supports the root `/` path for now.

```yaml
ingress:
  annotations:
    cert-manager.io/cluster-issuer: letsencrypt-prod
    external-dns.alpha.kubernetes.io/hostname: sample.mpw.sh
    external-dns.alpha.kubernetes.io/target: ing.pmw.sh
  className: nginx
  domains:
    shared: sample.mpw.sh
    custom: ""
```

### Environment Variables

Environment variables for your application can be set under this section.
Entries found here will be saved in a `configMap` and mounted as environment variables on your deployment.

```yaml
envVars:
  key1: value1
  key2: value2
```

### Secrets

Same as `EnvVars`

```yaml
secrets:
  secretKey1: secretValue1
  secretKey2: secretValue2
```

Here is a complete example of a `TailoredApp` manifest:

```yaml
apiVersion: mpw.sh/v1
kind: TailoredApp
metadata:
  name: example-tapp
spec:
  labels:
    owner: an-owner
    tapp: example-tapp
  ingress:
    className: nginx
    domains:
      shared: sample.mpw.sh
      custom: ""
  deployment:
    container:
      image: nginx:latest
      port: 80
      replicas: 2
      volumes:
        /home/test/123: 40Mi
        /home/test/anotherone: 100Mi
      fileMounts:
        /etc/config.toml: |-
          [someconfig]
          avalue= true
  envVars:
    key1: value1
    key2: value2
  secrets:
    secretKey1: secretValue1
    secretKey2: secretValue2
```

Remember to replace values in the sample manifest with your actual application details.
For more details about each field, please refer to the [Custom Resource Definition](./deploy/crd.yaml) on the [deploy](./deploy) folder.
