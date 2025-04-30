FROM almalinux:9.5-20250411 AS base

ARG NPROC=8

# Install tools and dependencies
# libgit2 (Dockerfile): cmake
# Kafka (Dockerfile): java-11-openjdk
# OpenSSL 3.2.2 (Dockerfile): perl-FindBin, perl-IPC-Cmd, perl-Pod-Html
RUN dnf update -y \
    && dnf group install -y "Development Tools" \
    && dnf install -y cmake java-11-openjdk perl-FindBin perl-IPC-Cmd perl-Pod-Html

# Install crates-pro test/runtime dependencies
ENV KAFKA_VERSION=3.9.0
ENV SCALA_VERSION=2.13
ENV KAFKA_HOME=/opt/kafka
RUN curl -O https://downloads.apache.org/kafka/${KAFKA_VERSION}/kafka_${SCALA_VERSION}-${KAFKA_VERSION}.tgz && \
    tar -xzf kafka_${SCALA_VERSION}-${KAFKA_VERSION}.tgz -C /opt && \
    mv /opt/kafka_${SCALA_VERSION}-${KAFKA_VERSION} $KAFKA_HOME && \
    rm kafka_${SCALA_VERSION}-${KAFKA_VERSION}.tgz

# Install OpenSSL 3.2.2 from source
# Required by: libgit2 1.9.0 (Dockerfile)
WORKDIR /tmp
RUN curl -LO https://www.openssl.org/source/openssl-3.2.2.tar.gz \
    && tar -xzf openssl-3.2.2.tar.gz \
    && cd openssl-3.2.2 \
    && ./config --prefix=/usr/local/ssl --openssldir=/usr/local/ssl shared enable-sm4 \
    && make -j$(NPROC) \
    && make install \
    && echo "/usr/local/ssl/lib64" > /etc/ld.so.conf.d/openssl.conf \
    && ldconfig \
    && ln -sf /usr/local/ssl/bin/openssl /usr/bin/openssl

# Install libgit2 1.9.0 from source
# Required by: ./crates_pro, ./bin_analyze, ./bin_data_transport, ./bin_repo_import
WORKDIR /tmp
RUN curl -LO https://github.com/libgit2/libgit2/archive/refs/tags/v1.9.0.tar.gz \
    && tar -xzf v1.9.0.tar.gz \
    && cd libgit2-1.9.0 \
    && mkdir build && cd build \
    && cmake .. -DCMAKE_INSTALL_PREFIX=/usr/local \
    -DOPENSSL_ROOT_DIR=/usr/local/ssl \
    -DBUILD_SHARED_LIBS=ON \
    && make -j$(NPROC) \
    && make install \
    && ldconfig

# Set library environment variables
ENV LD_LIBRARY_PATH="/usr/local/ssl/lib64:/usr/local/lib64:/usr/lib64" \
    PKG_CONFIG_PATH="/usr/local/ssl/lib64/pkgconfig:/usr/local/lib64/pkgconfig:/usr/lib64/pkgconfig"

# Create and switch to user
ARG USERNAME="rust"
ARG USER_UID="1000"
RUN useradd -m -s /bin/bash -u $USER_UID $USERNAME \
    && mkdir -p /etc/sudoers.d \
    && echo $USERNAME ALL=\(root\) NOPASSWD:ALL > /etc/sudoers.d/$USERNAME \
    && chmod 0440 /etc/sudoers.d/$USERNAME
USER $USERNAME

# Create and set permissions for directories
USER root
RUN mkdir -p /workdir && chown $USERNAME:$USERNAME /workdir
RUN chown -R $USERNAME:$USERNAME $KAFKA_HOME
USER $USERNAME

WORKDIR /workdir

# Download resources required by `//third-party/vendor/utoipa-swagger-ui-9.0.1-patch1/src/lib.rs`
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
