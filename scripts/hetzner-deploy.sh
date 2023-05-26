#!/bin/bash
#Hetzner API TOKEN
API_TOKEN="your-api-token"
context_name=mpwsh

ip_range="10.0.0.0/16"
network_name="k3s-network"
network_zone="eu-central"
ssh_key_name=mpwsh
ssh_key_path=~/.ssh/hetzner.pub
firewall_name="k3s-firewall"
placement_group="spread-kubes"

#download hcloud CLI from https://github.com/hetznercloud/cli

#Use context
hcloud context use $context_name
#Create SSH KEY
hcloud ssh-key create --name $ssh_key_name --public-key-from-file ~/.ssh/hetzner.pub

#First, we'll create a private network which is used by our Kubernetes nodes for communicating with each other. We'll use 10.0.0.0/16 as network and subnet.
hcloud network create --name $network_name --ip-range $ip_range
hcloud network add-subnet $network_name --network-zone $network_zone --type server --ip-range $ip_range

#The placement group ensures your VMs run on different hosts, so in case on host has a failure, no other VMs are affected.
hcloud placement-group create --name $placement_group --type spread

hcloud server create --datacenter fsn1-dc14 --type cax21 --name main-1 --image debian-11 --ssh-key $ssh_key_path --network $network_name --placement-group $placement_group
hcloud server create --datacenter fsn1-dc14 --type cax11 --name node-1 --image debian-11 --ssh-key $ssh_key_path --network $network_name --placement-group $placement_group
hcloud server create --datacenter fsn1-dc14 --type cax11 --name node-2 --image debian-11 --ssh-key $ssh_key_path --network $network_name --placement-group $placement_group


#Create firewall
hcloud firewall create --name $firewall_name
hcloud firewall add-rule $firewall_name --description "Allow SSH In" --direction in --port 22 --protocol tcp --source-ips 0.0.0.0/0 --source-ips ::/0
hcloud firewall add-rule $firewall_name --description "Allow ICMP In" --direction in --protocol icmp --source-ips 0.0.0.0/0 --source-ips ::/0
hcloud firewall add-rule $firewall_name --description "Allow Kube API In" --direction in --port 6443 --protocol tcp --source-ips 0.0.0.0/0 --source-ips ::/0


# Add rules
hcloud firewall add-rule $firewall_name --description "Allow ICMP Out" --direction out --protocol icmp --destination-ips 0.0.0.0/0 --destination-ips ::/0
hcloud firewall add-rule $firewall_name --description "Allow DNS TCP Out" --direction out --port 53 --protocol tcp --destination-ips 0.0.0.0/0 --destination-ips ::/0
hcloud firewall add-rule $firewall_name --description "Allow DNS UDP Out" --direction out --port 53 --protocol udp --destination-ips 0.0.0.0/0 --destination-ips ::/0
hcloud firewall add-rule $firewall_name --description "Allow HTTP Out" --direction out --port 80 --protocol tcp --destination-ips 0.0.0.0/0 --destination-ips ::/0
hcloud firewall add-rule $firewall_name --description "Allow HTTPS Out" --direction out --port 443 --protocol tcp --destination-ips 0.0.0.0/0 --destination-ips ::/0
hcloud firewall add-rule $firewall_name --description "Allow NTP UDP Out" --direction out --port 123 --protocol udp --destination-ips 0.0.0.0/0 --destination-ips ::/0
hcloud firewall add-rule $firewall_name --description "Allow Kube API Out" --direction out --port 6443 --protocol tcp --destination-ips 0.0.0.0/0 --destionation-ips ::/0


#Apply to servers
hcloud firewall apply-to-resource $firewall_name --type server --server main-1
hcloud firewall apply-to-resource $firewall_name --type server --server node-1
hcloud firewall apply-to-resource $firewall_name --type server --server node-2


# Install initial-deps in all nodes + master.
./initial-deps.sh

#Deploy master with
./master-init.sh


#Deploy nodes with:
./node-init.sh

# Add or validate the following cilium config-entries under the cilium-config configmap:
# enable-endpoint-routes: "true"
# native-routing-cidr: "10.244.0.0/16"
# ipam: kubernetes
# tunnel: geneve

kubectl delete secret hcloud -n kube-system && kubectl -n kube-system create secret generic hcloud --from-literal="token=$API_TOKEN" --from-literal="network=$network_name"
kubectl apply -f https://github.com/hetznercloud/hcloud-cloud-controller-manager/releases/latest/download/ccm-networks.yaml


#If you get errors when asking for load balancer from a service, check that all nodes contain the `providerID` ID in the spec:
#spec:
#    providerID: hcloud://32879444
