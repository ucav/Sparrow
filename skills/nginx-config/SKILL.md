# Skill: NGINX Config

**Trigger:** nginx, reverse proxy, web server, SSL, HTTPS

**Description:** Configuration NGINX : reverse proxy, SSL/TLS, load balancing, caching, sécurité.

## Body

### Reverse proxy standard
```nginx
server {
    listen 443 ssl http2;
    server_name example.com;

    ssl_certificate     /etc/letsencrypt/live/example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/example.com/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:9339;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}

server {
    listen 80;
    server_name example.com;
    return 301 https://$host$request_uri;  # Redirect HTTP→HTTPS
}
```

### Rate limiting
```nginx
limit_req_zone $binary_remote_addr zone=mylimit:10m rate=10r/s;

location /api/ {
    limit_req zone=mylimit burst=20 nodelay;
    proxy_pass http://backend;
}
```

### Cache
```nginx
proxy_cache_path /var/cache/nginx levels=1:2 keys_zone=mycache:10m;

location / {
    proxy_cache mycache;
    proxy_cache_valid 200 10m;
    proxy_cache_key "$scheme$request_method$host$request_uri";
}
```

### Commandes
```bash
nginx -t                    # Vérifier la config
nginx -s reload             # Recharger sans downtime
tail -f /var/log/nginx/error.log
```

### Pièges
- `proxy_pass` sans trailing slash → chemins cassés
- `client_max_body_size` trop petit → uploads bloqués
- SSL config faible → tester avec https://sslabs.com
