# Skill: Docker Management
**Trigger:** docker, container, image, compose, deploy container
**Description:** Manage Docker containers — build, run, compose, debug, optimize.

## Body
1. Check Docker daemon: docker info
2. List resources: docker ps, docker images, docker volumes
3. Build images: docker build -t name:tag . with proper .dockerignore
4. Run containers: docker run with appropriate flags (-d, -p, -v, -e)
5. Docker Compose: docker compose up -d, docker compose logs
6. Clean up: docker system prune -a (careful!)
7. Debug: docker logs, docker exec -it <container> sh
