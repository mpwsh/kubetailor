#!/bin/bash
HOSTNAME=$(hostname -f)
PRIVATE_IP=$(ip route get 10.0.0.0/16 | awk -F"src " 'NR==1{split($2,a," ");print a[1]}')
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
--node-name="$HOSTNAME" \
--tls-san="$(hostname -I | awk '{print $2}')" \
--cluster-cidr=10.244.0.0/16 \
--etcd-expose-metrics=true \
--kube-controller-manager-arg="bind-address=0.0.0.0" \
--kube-proxy-arg="metrics-bind-address=0.0.0.0" \
--kube-scheduler-arg="bind-address=0.0.0.0" \
--kubelet-arg="cloud-provider=external" \
--advertise-address="$PRIVATE_IP" \
--node-ip="$PRIVATE_IP" \
--node-external-ip="$PUBLIC_IP"

systemctl start k3s
sleep 10

export KUBECONFIG=/etc/rancher/k3s/k3s.yaml
export KUBE_EDITOR=vim

CILIUM_CLI_VERSION=$(curl -s https://raw.githubusercontent.com/cilium/cilium-cli/master/stable.txt)
CLI_ARCH=amd64

if [ "$(uname -m)" = "aarch64" ]; then CLI_ARCH=arm64; fi
curl -L --fail -s --remote-name-all "https://github.com/cilium/cilium-cli/releases/download/${CILIUM_CLI_VERSION}/cilium-linux-${CLI_ARCH}.tar.gz{,.sha256sum}"
sha256sum --check cilium-linux-${CLI_ARCH}.tar.gz.sha256sum
sudo tar xzvfC cilium-linux-${CLI_ARCH}.tar.gz /usr/local/bin
rm cilium-linux-${CLI_ARCH}.tar.gz{,.sha256sum}

cilium install

kubectl taint nodes main-1 node.cloudprovider.kubernetes.io/uninitialized:NoSchedule-
echo "JOIN WITH TOKEN:"
cat /var/lib/rancher/k3s/server/token
echo "restarting"
shutdown -r now
