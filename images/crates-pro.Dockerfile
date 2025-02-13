ARG BASE_IMAGE
FROM $BASE_IMAGE AS builder

RUN buck2 clean
RUN mkdir -p /build

RUN cp "$(buck2 build //project/crates-pro:crates_pro --show-output | awk '{print $2}')" /build/crates_pro
RUN cp /workdir/project/crates-pro/.env /build/.env

FROM almalinux:8.10-20240923 AS runner

# Install tools and dependencies
# Kafka: java-11-openjdk
RUN dnf update -y \
    && dnf group install -y "Development Tools" \
    && dnf install -y curl java-11-openjdk

# Install crates-pro test/runtime dependencies
ENV KAFKA_VERSION=3.9.0
ENV SCALA_VERSION=2.13
ENV KAFKA_HOME=/opt/kafka
RUN curl -O https://downloads.apache.org/kafka/${KAFKA_VERSION}/kafka_${SCALA_VERSION}-${KAFKA_VERSION}.tgz && \
    tar -xzf kafka_${SCALA_VERSION}-${KAFKA_VERSION}.tgz -C /opt && \
    mv /opt/kafka_${SCALA_VERSION}-${KAFKA_VERSION} $KAFKA_HOME && \
    rm kafka_${SCALA_VERSION}-${KAFKA_VERSION}.tgz

WORKDIR /workdir

# Download resources required by `//third-party/vendor/utoipa-swagger-ui-9.0.0-patch1/src/lib.rs`
# See https://github.com/crates-pro/crates-pro-infra/tree/main/third-party#step-4-update-patches
RUN curl -L -o swagger-ui-5.17.14.zip https://github.com/swagger-api/swagger-ui/archive/refs/tags/v5.17.14.zip \
    && unzip -j swagger-ui-5.17.14.zip "swagger-ui-5.17.14/dist/*" -d ./swagger-ui-5.17.14-dist \
    && rm -f swagger-ui-5.17.14.zip

# Copy and configure crates-pro
COPY --from=builder /build/crates_pro ./crates_pro
COPY --from=builder /build/.env ./.env

# Required by //project/crates-pro/crates_pro/src/main.rs
RUN mkdir target

ENV RUST_BACKTRACE=1
CMD ["./crates_pro"]
