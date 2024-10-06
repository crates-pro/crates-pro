# Use the TuGraph compile environment as the base image
FROM tugraph/tugraph-compile-ubuntu18.04:latest

# Install necessary dependencies
RUN apt-get update && apt-get install -y curl build-essential clang git cmake net-tools

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

RUN rustup update stable

# Set environment variable to include cargo bin
ENV PATH="/root/.cargo/bin:${PATH}"

# Expose the port used by TuGraph services, adjust as necessary
EXPOSE 7070

# # Clone TuGraph-db source code, ensure your Docker environment has access to GitHub
# RUN git clone --recurse-submodules https://github.com/TuGraph-family/tugraph-db.git

# # Build TuGraph-db dependencies
# RUN cd tugraph-db && bash deps/build_deps.sh -j8

# # Build lgraph_server
# RUN cmake -S ./tugraph-db -B build && cmake --build build --target lgraph_server -j 4

# Create a workspace directory within the container for code development
WORKDIR /workspace

