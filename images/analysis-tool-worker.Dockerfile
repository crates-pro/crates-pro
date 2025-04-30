FROM almalinux:9.5-20250411 AS base

# Install tools and dependencies
RUN dnf update -y \
    && dnf install -y git \
    && dnf clean all

# Create and switch to user
ARG USERNAME="rust"
ARG USER_UID="1000"
RUN useradd -m -s /bin/bash -u $USER_UID $USERNAME \
    && mkdir -p /etc/sudoers.d \
    && echo $USERNAME ALL=\(root\) NOPASSWD:ALL > /etc/sudoers.d/$USERNAME \
    && chmod 0440 /etc/sudoers.d/$USERNAME
USER $USERNAME

# Create and set permissions for workdir directory
USER root
RUN mkdir -p /workdir && chown $USERNAME:$USERNAME /workdir
USER $USERNAME

WORKDIR /workdir

# Copy artifacts for analysis-tool-worker
COPY ./analysis_tool_worker ./analysis_tool_worker
COPY ./tools/ /var/tools/
COPY ./.env ./.env

# Copy artifacts for tool 'sensleak-rs'
COPY ./scan /var/tools/sensleak/scan
COPY ./gitleaks.toml /var/tools/sensleak/gitleaks.toml

USER root
RUN chown -R $USERNAME:$USERNAME /var/tools
USER $USERNAME

ENV RUST_BACKTRACE=1
CMD ["./analysis_tool_worker"]
