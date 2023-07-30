#!/bin/bash
CMD_ARRAY=$(skopeo inspect --config "docker://${CONTAINER_IMAGE}" | jq -r '.config.Cmd')

# Use jq to concatenate the array elements into a string with single quotes around any item with a space
IMAGE_COMMAND=$(echo "${CMD_ARRAY}" | jq -r 'map(if contains(" ") then "'"'"'"+.+"'"'"'" else . end) | join(" ")')

# Select the command to run based on the presence of RUN_COMMAND
cmd_to_run="${RUN_COMMAND:-$IMAGE_COMMAND}"
cmd_to_build="${BUILD_COMMAND:-'echo nothing to build'}"

BUILD_PATH=/app/build
SYNC_PATH=/src/git-sync

cat << EOF > /init/run.sh
#!/bin/sh
mkdir -p "${BUILD_PATH}"
/init/watchexec -v --workdir ${SYNC_PATH} -w /src --project-origin ${SYNC_PATH} --restart "cp -r -L ${SYNC_PATH} ${BUILD_PATH} && cd ${BUILD_PATH}/git-sync && ${cmd_to_build} && ${cmd_to_run}"
EOF

chmod +x /init/run.sh
cp /bin/watchexec /init/watchexec
cat /init/run.sh
