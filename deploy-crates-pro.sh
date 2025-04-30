#!/bin/bash
set -euxo pipefail

# source
CRATESPRO_DIR=/home/rust/crates-pro
INFRA_DIR=/home/rust/workspace/crates-pro-infra
# deployment
NAMESPACE=crates-pro
INSTANCE=test1
DEPLOYMENT=cratespro-backend-$INSTANCE
KAFKA_HOST=172.17.0.1:30092
TAKE_SNAPSHOT_BEFORE_REDEPLOY=0
# build
BUILD_DIR=$CRATESPRO_DIR/build
IMAGES_DIR=$CRATESPRO_DIR/images
TIMESTAMP=$(date +%Y%m%d-%H%M)
CRATESPRO_MAIN_IMAGE=localhost:30500/crates-pro:local-$TIMESTAMP
CRATESPRO_ANALYZE_IMAGE=localhost:30500/cratespro-analyze:local-$TIMESTAMP
CRATESPRO_DATA_TRANSPORT_IMAGE=localhost:30500/cratespro-datatransport:local-$TIMESTAMP
CRATESPRO_REPO_IMPORT_IMAGE=localhost:30500/cratespro-repoimport:local-$TIMESTAMP

### Preparation: Sync source directory
rsync --delete --archive $CRATESPRO_DIR/ $INFRA_DIR/project/crates-pro --exclude="/.git" --exclude="/buck-out" --exclude="/build" --exclude="/target"

### Step 1: Compile, then copy artifacts to $BUILD_DIR
mkdir -p $BUILD_DIR
rm -rf $BUILD_DIR/*
cd $INFRA_DIR
cp "$(buck2 build //project/crates-pro:crates_pro --show-simple-output)" $BUILD_DIR/crates_pro
cp "$(buck2 build //project/crates-pro:bin_analyze --show-simple-output)" $BUILD_DIR/bin_analyze
cp "$(buck2 build //project/crates-pro:bin_data_transport --show-simple-output)" $BUILD_DIR/bin_data_transport
cp "$(buck2 build //project/crates-pro:bin_repo_import --show-simple-output)" $BUILD_DIR/bin_repo_import
cp $CRATESPRO_DIR/.env $BUILD_DIR/.env
cd $CRATESPRO_DIR

### Step 2: Build Docker images
docker build --target crates_pro        -t $CRATESPRO_MAIN_IMAGE            -f $IMAGES_DIR/crates-pro.Dockerfile    $BUILD_DIR
docker build --target analyze           -t $CRATESPRO_ANALYZE_IMAGE         -f $IMAGES_DIR/crates-pro.Dockerfile    $BUILD_DIR
docker build --target data_transport    -t $CRATESPRO_DATA_TRANSPORT_IMAGE  -f $IMAGES_DIR/crates-pro.Dockerfile    $BUILD_DIR
docker build --target repo_import       -t $CRATESPRO_REPO_IMPORT_IMAGE     -f $IMAGES_DIR/crates-pro.Dockerfile    $BUILD_DIR

### Step 3: Push Docker images
docker push $CRATESPRO_MAIN_IMAGE
docker push $CRATESPRO_ANALYZE_IMAGE
docker push $CRATESPRO_DATA_TRANSPORT_IMAGE
docker push $CRATESPRO_REPO_IMPORT_IMAGE

### Step 4: Stop current containers
# Scale deployment to 0 replicas
kubectl scale deployment $DEPLOYMENT -n $NAMESPACE --replicas=0

# Wait until all pods are terminated
while kubectl get pods -n $NAMESPACE | grep $DEPLOYMENT > /dev/null; do
    sleep 5
done

# Take snapshot if enabled or if INSTANCE is "main"
if [ "$TAKE_SNAPSHOT_BEFORE_REDEPLOY" -eq 1 ] || [ "$INSTANCE" = "main" ]; then
    /home/rust/src/crates-pro-control/cpctl-snapshot $INSTANCE
fi

### Step 5: Set new images
kubectl set image deployment/$DEPLOYMENT -n $NAMESPACE crates-pro=$CRATESPRO_MAIN_IMAGE
kubectl set image deployment/$DEPLOYMENT -n $NAMESPACE analyze=$CRATESPRO_ANALYZE_IMAGE
kubectl set image deployment/$DEPLOYMENT -n $NAMESPACE data-transport=$CRATESPRO_DATA_TRANSPORT_IMAGE
kubectl set image deployment/$DEPLOYMENT -n $NAMESPACE repo-import=$CRATESPRO_REPO_IMPORT_IMAGE

# Wait until all kafka consumers are removed
CONSUMER_GROUP=instance-$INSTANCE-group
while docker run --rm -t bitnami/kafka -- kafka-consumer-groups.sh --bootstrap-server $KAFKA_HOST --group $CONSUMER_GROUP --describe | grep rdkafka > /dev/null; do
    sleep 5
done

### Step 6: Run new containers
# Scale deployment back to 1 replica
kubectl scale deployment $DEPLOYMENT -n $NAMESPACE --replicas=1
