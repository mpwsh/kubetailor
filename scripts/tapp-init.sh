#!/bin/bash
CMD_ARRAY=$(skopeo inspect --config "docker://${CONTAINER_IMAGE}" | jq -r '.config.Cmd')

# Use jq to concatenate the array elements into a string with single quotes around any item with a space
IMAGE_COMMAND=$(echo "${CMD_ARRAY}" | jq -r 'map(if contains(" ") then "'"'"'"+.+"'"'"'" else . end) | join(" ")')

# Select the command to run based on the presence of RUN_COMMAND
cmd_to_run="${RUN_COMMAND:-$IMAGE_COMMAND}"

cat << EOF > /init/run.sh
#!/bin/bash
/init/watchexec -vvv --workdir /src/git-sync -w /src --project-origin /src/git-sync --restart "cd /src/git-sync && ${BUILD_COMMAND} && ${cmd_to_run}"
EOF

chmod +x /init/run.sh
cp /bin/watchexec /init/watchexec
cat /init/run.sh
