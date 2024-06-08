#!/bin/bash
HOSTNAME=$(hostname -f)
PRIVATE_IP=$(hostname -I | awk '{print $2}')
PUBLIC_IP=$(hostname -I | awk '{print $1}')

curl -sfL https://get.k3s.io | sh -s - server \
	--disable-cloud-controller \
	--disable servicelb \
	--disable traefik \
	--disable local-storage \
	--disable metrics-server \
	--disable-kube-proxy \
	--disable-network-policy \
	--flannel-backend=none \
	--write-kubeconfig-mode=644 \
	--node-name="${HOSTNAME}" \
	--tls-san="${PUBLIC_IP}" \
	--cluster-cidr=10.244.0.0/16 \
	--service-cidr=10.43.0.0/16 \
	--etcd-expose-metrics=true \
	--kube-controller-manager-arg="bind-address=0.0.0.0" \
	--kube-proxy-arg="metrics-bind-address=0.0.0.0" \
	--kube-scheduler-arg="bind-address=0.0.0.0" \
	--kubelet-arg="cloud-provider=external" \
	--advertise-address="${PRIVATE_IP}" \
	--node-ip="${PRIVATE_IP}" \
	--node-external-ip="${PUBLIC_IP}"

systemctl start k3s
sleep 10

export KUBECONFIG=/etc/rancher/k3s/k3s.yaml
export KUBE_EDITOR=vim
export HCLOUD_TOKEN="<your-token>"
network_name="k3s-network"
# Deploy cloud controller
kubectl -n kube-system create secret generic hcloud --from-literal="token=$HCLOUD_TOKEN" --from-literal="network=$network_name"
kubectl apply -f https://github.com/hetznercloud/hcloud-cloud-controller-manager/releases/latest/download/ccm-networks.yaml

CILIUM_CLI_VERSION=$(curl -s https://raw.githubusercontent.com/cilium/cilium-cli/master/stable.txt)
CLI_ARCH=amd64

if [ "$(uname -m)" = "aarch64" ]; then CLI_ARCH=arm64; fi
curl -L --fail -s --remote-name-all "https://github.com/cilium/cilium-cli/releases/download/${CILIUM_CLI_VERSION}/cilium-linux-${CLI_ARCH}.tar.gz{,.sha256sum}"
sha256sum --check cilium-linux-${CLI_ARCH}.tar.gz.sha256sum
sudo tar xzvfC cilium-linux-${CLI_ARCH}.tar.gz /usr/local/bin
rm cilium-linux-${CLI_ARCH}.tar.gz{,.sha256sum}

cilium install

kubectl patch node main-1 -p '{"spec":{"taints":[]}}'

sleep 30

cilium status
echo "JOIN WITH TOKEN:"
cat /var/lib/rancher/k3s/server/token
echo "P"
shutdown -r now
