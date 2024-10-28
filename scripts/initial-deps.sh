#!/bin/bash
sudo systemctl enable unattended-upgrades
sudo systemctl start unattended-upgrades
apt update
apt install nfs-common open-iscsi apparmor apparmor-utils vim -y
systemctl start open-iscsi
systemctl enable open-iscsi
apt upgrade -y
apt autoremove -y
