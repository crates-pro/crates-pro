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

ENV RUST_BACKTRACE=1

# 'crates_pro' image
FROM base AS crates_pro
COPY ./crates_pro ./crates_pro
COPY ./.env ./.env
# Required by //project/crates-pro/crates_pro/src/main.rs
RUN mkdir target
CMD ["./crates_pro"]

# 'analyze' image
FROM base AS analyze
COPY ./bin_analyze ./bin_analyze
COPY ./.env ./.env
CMD ["./bin_analyze"]

# 'data_transport' image
FROM base AS data_transport
COPY ./bin_data_transport ./bin_data_transport
COPY ./.env ./.env
CMD ["./bin_data_transport"]

# 'repo_import' image
FROM base AS repo_import
COPY ./bin_repo_import ./bin_repo_import
COPY ./.env ./.env
CMD ["./bin_repo_import"]
