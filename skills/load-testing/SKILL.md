# Skill: Load Testing

**Trigger:** load test, benchmark, stress test, performance test

**Description:** Test de charge avec k6 : scénarios, métriques, analyse de bottlenecks.

## Body

### k6 Quick Start
```bash
npm install -g k6
k6 run test.js
```

### Script k6 standard
```javascript
import http from 'k6/http';
import { check, sleep } from 'k6';

export const options = {
  stages: [
    { duration: '30s', target: 20 },  // monter à 20 users
    { duration: '1m', target: 20 },   // maintenir 1 min
    { duration: '30s', target: 0 },   // redescendre
  ],
  thresholds: {
    http_req_duration: ['p(95)<500'], // 95% des requêtes < 500ms
    http_req_failed: ['rate<0.01'],   // <1% d'erreurs
  },
};

export default function () {
  const res = http.get('http://localhost:9339/status');
  check(res, { 'status is 200': (r) => r.status === 200 });
  sleep(1);
}
```

### Métriques clés
- **p95/p99 latency** — pas la moyenne, les queues de distribution
- **Throughput** — req/s soutenu
- **Error rate** — % de 5xx/4xx
- **Concurrent users** — combien avant dégradation

### Interpréter les résultats
```
http_req_duration: avg=150ms p(95)=450ms p(99)=800ms
→ OK, sous les 500ms pour 95% des requêtes

http_req_duration: avg=2s p(95)=5s p(99)=10s
→ PROBLÈME : identifier le bottleneck (DB? CPU? réseau?)
```

### Pièges
- Tester en local → résultats non représentatifs de la prod
- Oublier le warm-up → les premières requêtes sont lentes (cold start)
- Un seul scénario → simuler des vrais parcours utilisateur
