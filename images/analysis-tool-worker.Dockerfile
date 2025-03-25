FROM almalinux:8.10-20250307

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

# Copy artifacts of tool 'sensleak-rs'
COPY ./scan ./scan
COPY ./gitleaks.toml ./gitleaks.toml

# Copy artifacts of worker (analysis-tool-worker)
COPY ./analysis_tool_worker ./analysis_tool_worker
COPY ./.env ./.env

ENV RUST_BACKTRACE=1
CMD ["./analysis_tool_worker"]
