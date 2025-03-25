#!/bin/bash
set -euxo pipefail

# source
INFRA_PATH=/home/rust/workspace/crates-pro-infra
LATEST_SRC_PATH=.
# deployment
NAMESPACE=crates-pro
INSTANCE=test1
DEPLOYMENT=cratespro-backend-$INSTANCE
KAFKA_HOST=172.17.0.1:30092
TAKE_SNAPSHOT_BEFORE_REDEPLOY=0
# build
TIMESTAMP=$(date +%Y%m%d-%H%M)
STAGE1_IMAGE=crates-pro-infra:base-$TIMESTAMP
STAGE2_IMAGE=crates-pro-infra:override-crates-pro-$TIMESTAMP
CRATESPRO_MAIN_IMAGE=localhost:30500/crates-pro:local-$TIMESTAMP
CRATESPRO_ANALYZE_IMAGE=localhost:30500/crates-pro-analyze:local-$TIMESTAMP
CRATESPRO_DATA_TRANSPORT_IMAGE=localhost:30500/crates-pro-data-transport:local-$TIMESTAMP
CRATESPRO_REPO_IMPORT_IMAGE=localhost:30500/crates-pro-repo-import:local-$TIMESTAMP

docker build -t $STAGE1_IMAGE \
    -f $INFRA_PATH/images/base.Dockerfile \
    $INFRA_PATH
docker build -t $STAGE2_IMAGE \
    -f $INFRA_PATH/images/override-crates-pro.Dockerfile \
    --build-arg BASE_IMAGE=$STAGE1_IMAGE \
    $LATEST_SRC_PATH
docker image rm $STAGE1_IMAGE
docker build --target crates_pro -t $CRATESPRO_MAIN_IMAGE \
    -f $LATEST_SRC_PATH/images/crates-pro.Dockerfile \
    --build-arg BASE_IMAGE=$STAGE2_IMAGE \
    --build-arg http_proxy --build-arg https_proxy \
    --ulimit nofile=65535:65535 \
    $LATEST_SRC_PATH
docker build --target analyze -t $CRATESPRO_ANALYZE_IMAGE \
    -f $LATEST_SRC_PATH/images/crates-pro.Dockerfile \
    --build-arg BASE_IMAGE=$STAGE2_IMAGE \
    --build-arg http_proxy --build-arg https_proxy \
    --ulimit nofile=65535:65535 \
    $LATEST_SRC_PATH
docker build --target data_transport -t $CRATESPRO_DATA_TRANSPORT_IMAGE \
    -f $LATEST_SRC_PATH/images/crates-pro.Dockerfile \
    --build-arg BASE_IMAGE=$STAGE2_IMAGE \
    --build-arg http_proxy --build-arg https_proxy \
    --ulimit nofile=65535:65535 \
    $LATEST_SRC_PATH
docker build --target repo_import -t $CRATESPRO_REPO_IMPORT_IMAGE \
    -f $LATEST_SRC_PATH/images/crates-pro.Dockerfile \
    --build-arg BASE_IMAGE=$STAGE2_IMAGE \
    --build-arg http_proxy --build-arg https_proxy \
    --ulimit nofile=65535:65535 \
    $LATEST_SRC_PATH
docker image rm $STAGE2_IMAGE
docker push $CRATESPRO_MAIN_IMAGE
docker push $CRATESPRO_ANALYZE_IMAGE
docker push $CRATESPRO_DATA_TRANSPORT_IMAGE
docker push $CRATESPRO_REPO_IMPORT_IMAGE

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

# Set new images
kubectl set image deployment/$DEPLOYMENT -n $NAMESPACE crates-pro=$CRATESPRO_MAIN_IMAGE
kubectl set image deployment/$DEPLOYMENT -n $NAMESPACE analyze=$CRATESPRO_ANALYZE_IMAGE
kubectl set image deployment/$DEPLOYMENT -n $NAMESPACE data-transport=$CRATESPRO_DATA_TRANSPORT_IMAGE
kubectl set image deployment/$DEPLOYMENT -n $NAMESPACE repo-import=$CRATESPRO_REPO_IMPORT_IMAGE

# Wait until all kafka consumers are removed
CONSUMER_GROUP=instance-$INSTANCE-group
while docker run --rm -t bitnami/kafka -- kafka-consumer-groups.sh --bootstrap-server $KAFKA_HOST --group $CONSUMER_GROUP --describe | grep rdkafka > /dev/null; do
    sleep 5
done

# Scale deployment back to 1 replica
kubectl scale deployment $DEPLOYMENT -n $NAMESPACE --replicas=1
