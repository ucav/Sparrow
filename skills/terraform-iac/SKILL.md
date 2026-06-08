# Skill: Terraform IAC

**Trigger:** terraform, iac, infrastructure, AWS infra

**Description:** Terraform : HCL, modules, state, plan/apply, AWS provider.

## Body

```bash
terraform init        # Initialiser le backend
terraform plan        # Voir les changements
terraform apply       # Appliquer
terraform destroy     # Détruire
terraform fmt -recursive  # Formater
```

### Exemple : EC2 + S3
```hcl
provider "aws" {
  region = "eu-west-1"
}

resource "aws_s3_bucket" "data" {
  bucket = "sparrow-data-${terraform.workspace}"
  acl    = "private"
}

resource "aws_instance" "app" {
  ami           = "ami-0c55b159cbfafe1f0"
  instance_type = "t3.micro"

  tags = {
    Name = "sparrow-${terraform.workspace}"
  }
}

output "app_ip" {
  value = aws_instance.app.public_ip
}
```

### State management
```hcl
terraform {
  backend "s3" {
    bucket = "sparrow-tfstate"
    key    = "prod/terraform.tfstate"
    region = "eu-west-1"
  }
}
```

### Pièges
- `terraform destroy` en prod = catastrophe → toujours vérifier le workspace
- State local → perdu si le laptop crash → utiliser backend S3
- Secrets dans le state → utiliser `sensitive = true` ou variables d'env
- `count` vs `for_each` → `for_each` préserve l'ordre après suppression
