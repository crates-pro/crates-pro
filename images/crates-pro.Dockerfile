ARG BASE_IMAGE
FROM $BASE_IMAGE AS builder

RUN buck2 clean
RUN mkdir -p /build

RUN cp "$(buck2 build //project/crates-pro:crates_pro --show-simple-output)" /build/crates_pro \
    && cp "$(buck2 build //project/crates-pro:bin_analyze --show-simple-output)" /build/bin_analyze \
    && cp "$(buck2 build //project/crates-pro:bin_data_transport --show-simple-output)" /build/bin_data_transport \
    && cp "$(buck2 build //project/crates-pro:bin_repo_import --show-simple-output)" /build/bin_repo_import
RUN cp /workdir/project/crates-pro/.env /build/.env

FROM almalinux:8.10-20250307 AS base

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

# Required by //project/crates-pro/crates_pro/src/main.rs
RUN mkdir target

ENV RUST_BACKTRACE=1

# crates_pro image
FROM base AS crates_pro
COPY --from=builder /build/crates_pro ./crates_pro
COPY --from=builder /build/.env ./.env
CMD ["./crates_pro"]

# analyze image
FROM base AS analyze
COPY --from=builder /build/bin_analyze ./bin_analyze
COPY --from=builder /build/.env ./.env
CMD ["./bin_analyze"]

# data_transport image
FROM base AS data_transport
COPY --from=builder /build/bin_data_transport ./bin_data_transport
COPY --from=builder /build/.env ./.env
CMD ["./bin_data_transport"]

# repo_import image
FROM base AS repo_import
COPY --from=builder /build/bin_repo_import ./bin_repo_import
COPY --from=builder /build/.env ./.env
CMD ["./bin_repo_import"]
