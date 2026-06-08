# Skill: AWS S3

**Trigger:** s3, AWS storage, bucket, upload, presigned

**Description:** AWS S3 : création de bucket, upload/download, presigned URLs, politiques de sécurité, CLI.

## Body

```bash
# CLI
aws s3 ls                                  # Lister les buckets
aws s3 ls s3://my-bucket/                  # Contenu
aws s3 cp file.txt s3://my-bucket/         # Upload
aws s3 cp s3://my-bucket/file.txt .        # Download
aws s3 sync ./local s3://my-bucket/        # Sync
aws s3 presign s3://my-bucket/file.txt     # URL temporaire
```

### Rust (aws-sdk-s3)
```rust
let config = aws_config::load_from_env().await;
let client = aws_sdk_s3::Client::new(&config);

// Upload
client.put_object()
    .bucket("my-bucket")
    .key("file.txt")
    .body(ByteStream::from(data))
    .send().await?;

// Download
let resp = client.get_object()
    .bucket("my-bucket")
    .key("file.txt")
    .send().await?;
let data = resp.body.collect().await?;
```

### Politique de bucket (public read)
```json
{
    "Version": "2012-10-17",
    "Statement": [{
        "Effect": "Allow",
        "Principal": "*",
        "Action": "s3:GetObject",
        "Resource": "arn:aws:s3:::my-bucket/*"
    }]
}
```

### Pièges
- Bucket public sans restriction → fuite de données
- Presigned URL avec expiration trop longue → risque sécurité
- `aws s3 sync` sans `--delete` → fichiers orphelins
