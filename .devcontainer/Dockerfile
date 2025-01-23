# Use the TuGraph compile environment as the base image, specifically 'tugraph/tugraph-compile-centos8', to avoid
# compatibility issues. It has a newer version of GLIBC (2.28) than 'tugraph/tugraph-compile-ubuntu18.04' (2.27).
FROM tugraph/tugraph-compile-centos8:1.3.4

# Install dependencies
RUN dnf update -y \
    && dnf group install -y "Development Tools" \
    && dnf install -y glibc-langpack-en sudo tzdata \
    && dnf install -y clang cmake curl git net-tools

# Install Docker
ARG DOCKER_GID
RUN groupadd -g $DOCKER_GID docker
RUN dnf config-manager --add-repo=https://download.docker.com/linux/centos/docker-ce.repo \
    && dnf install -y docker-ce docker-ce-cli containerd.io

# Install kubectl
RUN curl -LO "https://dl.k8s.io/release/v1.28.12/bin/linux/amd64/kubectl" \
    && chmod +x kubectl \
    && mv kubectl /usr/local/bin/

# Create and switch to user
ARG USERNAME
ARG USER_UID
RUN useradd -m -s /bin/bash -u $USER_UID $USERNAME \
    && mkdir -p /etc/sudoers.d \
    && echo $USERNAME ALL=\(root\) NOPASSWD:ALL > /etc/sudoers.d/$USERNAME \
    && chmod 0440 /etc/sudoers.d/$USERNAME \
    && usermod -aG docker $USERNAME
USER $USERNAME

# Install Rust, set environment variable
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/home/$USERNAME/.cargo/bin:${PATH}"

# Create and set permissions for workspace directory
USER root
RUN mkdir -p /workspace && chown $USERNAME:$USERNAME /workspace
USER $USERNAME

WORKDIR /workspace

ENV TZ=Asia/Shanghai
ENV LANG=en_US.UTF-8
ENV LC_ALL=en_US.UTF-8
