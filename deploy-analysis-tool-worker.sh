#!/bin/bash
set -euxo pipefail

# source
INFRA_PATH=/var/crates-pro-infra
# deployment
NAMESPACE=analysis-tool-worker
DEPLOYMENT=analysis-tool-worker
KAFKA_HOST=172.17.0.1:30092
# build
BUILD_DIR=/workspace/build
IMAGES_DIR=/workspace/images
TIMESTAMP=$(date +%Y%m%d-%H%M)
ANALYSIS_TOOL_WORKER_IMAGE=localhost:30500/analysis-tool-worker:local-$TIMESTAMP

### Step 1: Compile, then copy artifacts to $BUILD_DIR
mkdir -p $BUILD_DIR
rm -rf $BUILD_DIR/*
cd $INFRA_PATH
# Copy artifacts of tool 'sensleak-rs'
cp "$(buck2 build //project/sensleak-rs:scan --show-simple-output)" $BUILD_DIR/scan
cp $INFRA_PATH/project/sensleak-rs/gitleaks.toml $BUILD_DIR/gitleaks.toml
# Copy artifacts of worker (analysis-tool-worker)
cp "$(buck2 build //project/crates-pro/analysis:analysis_tool_worker --show-simple-output)" $BUILD_DIR/analysis_tool_worker
cp /workspace/.env $BUILD_DIR/.env
cd /workspace

### Step 2: Build Docker images
docker build -t $ANALYSIS_TOOL_WORKER_IMAGE -f $IMAGES_DIR/analysis-tool-worker.Dockerfile $BUILD_DIR

### Step 3: Push Docker images
docker push $ANALYSIS_TOOL_WORKER_IMAGE

### Step 4: Stop current containers
# Scale deployment to 0 replicas
kubectl scale deployment $DEPLOYMENT -n $NAMESPACE --replicas=0

# Wait until all pods are terminated
while kubectl get pods -n $NAMESPACE | grep $DEPLOYMENT > /dev/null; do
    sleep 5
done

### Step 5: Set new images
kubectl set image deployment/$DEPLOYMENT -n $NAMESPACE container-0=$ANALYSIS_TOOL_WORKER_IMAGE

# Wait until all kafka consumers are removed
CONSUMER_GROUP=analysis-tool-worker-1-group
while docker run --rm -t bitnami/kafka -- kafka-consumer-groups.sh --bootstrap-server $KAFKA_HOST --group $CONSUMER_GROUP --describe | grep rdkafka > /dev/null; do
    sleep 5
done

### Step 6: Run new containers
# Scale deployment back to 1 replica
kubectl scale deployment $DEPLOYMENT -n $NAMESPACE --replicas=1
