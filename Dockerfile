# --- Stage 1: Build the Rust Application ---
FROM rust:1.81-slim-bullseye AS builder

WORKDIR /app
COPY Cargo.toml ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release
COPY src ./src
RUN touch src/main.rs && cargo build --release

# --- Stage 2: Create the Final, Minimal Runtime Image ---
FROM debian:bullseye-slim AS final

RUN groupadd --system app && useradd --system --gid app appuser
USER appuser
WORKDIR /home/appuser/app

COPY --from=builder /app/target/release/Raytracer .
#COPY scene.json .
RUN mkdir renders

# Use ENTRYPOINT to define the main executable.
ENTRYPOINT ["./Raytracer"]

# Use CMD to provide default arguments (in this case, none).
# Arguments from `docker run` will be a