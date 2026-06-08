# Skill: Docker Management

**Trigger:** docker, container, image, compose, build

**Description:** Gestion Docker : Dockerfile, docker-compose, multi-stage builds, optimisation taille, debugging conteneurs.

## Body

### Commandes essentielles
```bash
docker ps                    # Conteneurs actifs
docker ps -a                 # Tous les conteneurs
docker images                # Images locales
docker logs -f <container>   # Logs en direct
docker exec -it <container> sh  # Shell dans le conteneur
```

### Dockerfile optimisé (multi-stage Rust)
```dockerfile
# Stage 1: Build
FROM rust:1.96 AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release  # Cache les dépendances
COPY src src
RUN touch src/main.rs && cargo build --release

# Stage 2: Runtime (6 MB)
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/sparrow /usr/local/bin/
ENTRYPOINT ["sparrow"]
```

### Docker Compose
```yaml
version: "3.8"
services:
  app:
    build: .
    ports: ["9339:9339"]
    environment:
      - NVIDIA_API_KEY=${NVIDIA_API_KEY}
    volumes:
      - ./data:/app/data
    restart: unless-stopped
```

### Debugging
```bash
docker logs --tail 100 -f sparrow     # Dernières 100 lignes
docker inspect sparrow | jq '.'        # Config complète
docker stats                            # CPU/RAM live
docker system prune -a                  # Nettoyage (attention !)
```

### Pièges
- `COPY . .` copie `target/`, `.git/`, fichiers sensibles → utiliser `.dockerignore`
- `latest` tag = non reproductible → toujours versionner
- Secrets dans Dockerfile → utiliser `--secret` ou variables d'env
