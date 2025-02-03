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
# build
NONCE=$(openssl rand -hex 4)
STAGE1_IMAGE=crates-pro-infra:base-$NONCE
STAGE2_IMAGE=crates-pro-infra:override-crates-pro-$NONCE
RUNNER_IMAGE=localhost:30500/crates-pro:local-$NONCE

docker build -t $STAGE1_IMAGE \
    -f $INFRA_PATH/images/base.Dockerfile \
    $INFRA_PATH
docker build -t $STAGE2_IMAGE \
    -f $INFRA_PATH/images/override-crates-pro.Dockerfile \
    --build-arg BASE_IMAGE=$STAGE1_IMAGE \
    $LATEST_SRC_PATH
docker image rm $STAGE1_IMAGE
docker build -t $RUNNER_IMAGE \
    -f $LATEST_SRC_PATH/images/crates-pro.Dockerfile \
    --build-arg BASE_IMAGE=$STAGE2_IMAGE \
    --ulimit nofile=65535:65535 \
    $LATEST_SRC_PATH
docker image rm $STAGE2_IMAGE
docker push $RUNNER_IMAGE

# Scale deployment to 0 replicas
kubectl scale deployment $DEPLOYMENT -n $NAMESPACE --replicas=0

# Wait until all pods are terminated
while kubectl get pods -n $NAMESPACE | grep $DEPLOYMENT > /dev/null; do
    sleep 5
done

# Set new image
kubectl set image deployment/$DEPLOYMENT -n $NAMESPACE crates-pro=$RUNNER_IMAGE

# Wait until all kafka consumers are removed
CONSUMER_GROUP=instance-$INSTANCE-group
while docker run --rm -t bitnami/kafka -- kafka-consumer-groups.sh --bootstrap-server $KAFKA_HOST --group $CONSUMER_GROUP --describe | grep rdkafka > /dev/null; do
    sleep 5
done

# Scale deployment back to 1 replica
kubectl scale deployment $DEPLOYMENT -n $NAMESPACE --replicas=1
