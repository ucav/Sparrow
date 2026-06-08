# Skill: Architecture Design

**Trigger:** architecture, design system, system design, microservices

**Description:** Conception d'architecture logicielle : choix de patterns, décomposition en services, trade-offs, diagrammes.

## Body

### Patterns par use case
```
API REST      → axum (Rust), FastAPI (Python), Express (Node)
Event-driven  → Kafka, RabbitMQ, NATS
CQRS          → séparer lectures/écritures, EventStore
Microservices → un service = une responsabilité, communiquer via API/events
Monolith first → 90% des projets n'ont pas besoin de microservices au début
```

### Questions à se poser avant de coder
1. Quel est le flux de données ? (user → API → DB → response)
2. Quels sont les goulets d'étranglement probables ?
3. Comment ça scale ? (vertical, horizontal, sharding)
4. Que se passe-t-il si ça crash ? (retry, circuit breaker, fallback)
5. Comment on debug ? (logs, traces, metrics)

### Stack Sparrow recommandée
```
Backend    : Rust (axum) ou Python (FastAPI)
Frontend   : React + TypeScript + Tailwind
Database   : PostgreSQL (relationnel) + Redis (cache)
Queue      : RabbitMQ ou Redis streams
Monitoring : Prometheus + Grafana
CI/CD      : GitHub Actions
```

### Pièges
- Over-engineering : pas de K8s pour un projet solo
- Microservices prématurés : commencer monolithe, splitter quand nécessaire
- Pas de monitoring : si tu peux pas mesurer, tu peux pas améliorer
