#!/bin/bash
BUCKET_ID=$(curl -X POST -sfL -H "Authorization: Bearer ${GARAGE_ADMIN_TOKEN}" --data @bucket.json localhost:3903/v1/bucket | jq -r .id)

echo "Created bucket with ID: ${BUCKET_ID}"
