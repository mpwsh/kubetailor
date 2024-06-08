#!/bin/bash
# Get cluster health
#curl -sfL -H "Authorization: Bearer ${GARAGE_ADMIN_TOKEN}" localhost:3903/v1/health | jq

# Get cluster status
#curl -sfL -H "Authorization: Bearer ${GARAGE_ADMIN_TOKEN}" localhost:3903/v1/status | jq

# Open layout.json and modify the node ids and capacity.
# Capacity is in bytes.
curl -X POST -sfL -H "Authorization: Bearer ${GARAGE_ADMIN_TOKEN}" --data @layout.json localhost:3903/v1/layout | jq

# Confirm staged layout
curl -sfL -H "Authorization: Bearer ${GARAGE_ADMIN_TOKEN}" localhost:3903/v1/layout | jq

# Apply layout
curl -X POST -sfL -H "Authorization: Bearer ${GARAGE_ADMIN_TOKEN}" localhost:3903/v1/layout/apply --data '{"version": 1}' | jq

#REVERT_LAYOUT=$(curl -X POST -sfL -H "Authorization: Bearer ${GARAGE_ADMIN_TOKEN}" localhost:3903/v1/layout/revert) --data '{"version": 1}'
#echo $REVERT_LAYOUT | jq
