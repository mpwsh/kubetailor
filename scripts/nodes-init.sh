#!/bin/bash
HOSTNAME=$(hostname -f)
MASTER_IP="10.0.0.2"
PRIVATE_IP="$(hostname -I | awk {'print $2'})"
#Get from master node in /var/lib/rancher/k3s/server/node-token
TOKEN="<join-token>"

curl -sfL https://get.k3s.io | K3S_TOKEN="$TOKEN" K3S_URL="https://$MASTER_IP:6443" sh -s - agent \
--node-name="$HOSTNAME" --kubelet-arg="cloud-provider=external" --node-ip="$PRIVATE_IP"

systemctl start k3s-agent
shutdown -r now
