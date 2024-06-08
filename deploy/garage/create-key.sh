#!/bin/bash
echo "Creating API KEY for quickwit"
CREATED_KEY=$(curl -X POST -sfL -H "Authorization: Bearer ${GARAGE_ADMIN_TOKEN}" --data '{"name": "quickwit"}' localhost:3903/v1/key | jq)

ACCESS_KEY_ID=$(echo ${CREATED_KEY} | jq -r .accessKeyId)
SECRET_ACCESS_KEY=$(echo ${CREATED_KEY} | jq -r .secretAccessKey)
echo "Access key:" ${ACCESS_KEY_ID}
echo "Secret access key:" ${SECRET_ACCESS_KEY}
