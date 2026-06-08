# Skill: K8s Deploy

**Trigger:** kubernetes, k8s, kubectl, deploy, pod, Helm

**Description:** Déploiement Kubernetes : manifests, kubectl, Helm, debugging pods, secrets.

## Body

```bash
kubectl get pods -n namespace        # Liste les pods
kubectl describe pod <name>          # Détails + events
kubectl logs -f <pod>                # Logs en streaming
kubectl exec -it <pod> -- sh         # Shell dans le pod
kubectl port-forward <pod> 8080:80   # Tunnel local
kubectl rollout restart deploy/app   # Redémarrage
```

### Deployment minimal
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: sparrow
spec:
  replicas: 2
  selector:
    matchLabels:
      app: sparrow
  template:
    metadata:
      labels:
        app: sparrow
    spec:
      containers:
      - name: sparrow
        image: ghcr.io/ucav/sparrow:v0.5.6
        ports:
        - containerPort: 9339
        env:
        - name: NVIDIA_API_KEY
          valueFrom:
            secretKeyRef:
              name: sparrow-secrets
              key: nvidia-key
        resources:
          requests:
            memory: "128Mi"
            cpu: "100m"
          limits:
            memory: "512Mi"
            cpu: "500m"
```

### Debugging
```bash
# Pod qui crash
kubectl describe pod sparrow-xxx | grep -A10 Events
kubectl logs sparrow-xxx --previous  # Logs du crash précédent

# ImagePullBackOff → vérifier le nom d'image et les credentials
kubectl get events --sort-by=.metadata.creationTimestamp
```

### Pièges
- `latest` tag → image pas mise à jour, utiliser des tags versionnés
- Secrets en clair dans les manifests → utiliser `secretKeyRef`
- `resources:` non définis → pod évincé par OOMKill
