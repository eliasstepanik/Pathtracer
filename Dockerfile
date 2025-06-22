# -----------------------------------------------------------------------------
# Stage 1: The Builder
#
# This stage uses the official Rust image, which contains the full compiler
# toolchain. Its purpose is to compile our Rust code into a static binary.
# -----------------------------------------------------------------------------
FROM rust:1.78-slim-bookworm AS builder

# Set the working directory inside the container
WORKDIR /usr/src/app

# --- Docker Caching Optimization ---
# 1. Create a dummy project to pre-build dependencies. This layer will only
#    be invalidated if Cargo.toml or Cargo.lock changes.
RUN cargo new --bin .

# 2. Copy the dependency manifests
COPY Cargo.toml Cargo.lock ./

# 3. Build only the dependencies. This is the step that takes a while but
#    will be cached on subsequent builds if dependencies don't change.
RUN cargo build --release
RUN rm src/main.rs # Clean up the dummy source file

# --- Actual Application Build ---
# 4. Copy your actual source code. If only your source code changes (not
#    dependencies), the build will be very fast from this point on.
COPY src ./src

# 5. Build the final binary with all optimizations.
#    Using --locked ensures reproducibility from Cargo.lock.
RUN cargo build --release --locked


# -----------------------------------------------------------------------------
# Stage 2: The Runner
#
# This stage creates the final, minimal image. We use a slim Debian base
# and copy only the compiled binary and necessary assets from the builder stage.
# The result is a much smaller and more secure image.
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runner

# It's a good practice to run as a non-root user for security
RUN groupadd --system app && useradd --system --gid app app
USER app

# Set the working directory for the non-root user
WORKDIR /home/app

# Create a directory for the output renders and set permissions
RUN mkdir renders && chown app:app renders

# Copy the scene configuration file into the image
COPY scene.json .

# Copy the compiled binary from the 'builder' stage.
# The binary name comes from `[package].name` in Cargo.toml.
COPY --from=builder /usr/src/app/target/release/Raytracer .

# Set the entrypoint for the container. When the container runs, it will
# execute this command.
ENTRYPOINT ["./Raytracer"]