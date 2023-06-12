#!/bin/bash
# Array of your Dockerfile targets
targets=("operator" "server" "console")

# Array of your image tags
images=("mpwsh/kt-operator:latest" "mpwsh/kt-server:latest" "mpwsh/kt-console:latest")

# Dockerfile path for the operator, server and console
dockerfilePath="./docker/Dockerfile"

# Ensure the length of targets array is equal to the length of images array
if [ "${#targets[@]}" -ne "${#images[@]}" ]; then
    echo "Error: targets and images arrays have different lengths."
    exit 1
fi

# Build and push images for operator, server, and console
for index in "${!images[@]}"; do
    echo "Building ${images[$index]} with target ${targets[$index]}"
    docker build -t "${images[$index]}" --target "${targets[$index]}" -f "$dockerfilePath" .
    echo "Pushing ${images[$index]}"
    docker push "${images[$index]}"
done

# Build and push image for initContainer separately
initImage="mpwsh/kt-init:latest"
initDockerfilePath="./docker/Dockerfile.initContainer"
echo "Building $initImage"
docker build -t "$initImage" -f "$initDockerfilePath" .
echo "Pushing $initImage"
docker push "$initImage"

echo "All images built and pushed successfully"
