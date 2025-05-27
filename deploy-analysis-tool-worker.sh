#!/bin/bash
set -euxo pipefail

# source
CRATESPRO_DIR=/home/rust/crates-pro
INFRA_DIR=/home/rust/workspace/crates-pro-infra
# deployment
NAMESPACE=analysis-tool-worker
DEPLOYMENT=analysis-tool-worker
KAFKA_HOST=172.17.0.1:30092
# build
BUILD_DIR=$CRATESPRO_DIR/build
IMAGES_DIR=$CRATESPRO_DIR/images
TIMESTAMP=$(date +%Y%m%d-%H%M)
ANALYSIS_TOOL_WORKER_IMAGE=localhost:30500/analysis-tool-worker:local-$TIMESTAMP

### Preparation: Sync source directory
rsync --delete --archive $CRATESPRO_DIR/ $INFRA_DIR/project/crates-pro --exclude="/.git" --exclude="/buck-out" --exclude="/build" --exclude="/target"

### Step 1: Compile, then copy artifacts to $BUILD_DIR
mkdir -p $BUILD_DIR
rm -rf $BUILD_DIR/*
cd $INFRA_DIR
# Copy artifacts for tool 'sensleak-rs'
cp "$(buck2 build //project/sensleak-rs:scan --show-simple-output)" $BUILD_DIR/scan
cp $INFRA_DIR/project/sensleak-rs/gitleaks.toml $BUILD_DIR/gitleaks.toml
# Copy artifacts for analysis-tool-worker
cp "$(buck2 build //project/crates-pro/analysis:analysis_tool_worker --show-simple-output)" $BUILD_DIR/analysis_tool_worker
cp -r $CRATESPRO_DIR/analysis/tools/ $BUILD_DIR/tools/
cp $CRATESPRO_DIR/.env $BUILD_DIR/.env
cd $CRATESPRO_DIR

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
