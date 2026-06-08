# Skill: Python Development

**Trigger:** python, pip, venv, pytest, Python

**Description:** Développement Python : venv, pytest, typing, ruff, packaging.

## Body

```bash
# Environnement
python3 -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt

# Tests
pytest -xvs                          # Stop au 1er échec, verbose
pytest --cov=src --cov-report=html   # Couverture
pytest -k "test_user"                # Filtre

# Qualité
ruff check . --fix                   # Lint + fix auto
black . && isort .                   # Format
mypy .                               # Type check
```

### Patterns
```python
# Typage
def get_user(user_id: int) -> User | None:
    ...

# Dataclass > dict
@dataclass
class Config:
    host: str = "localhost"
    port: int = 8080

# async/await
async def fetch_data(url: str) -> dict:
    async with httpx.AsyncClient() as client:
        resp = await client.get(url)
        return resp.json()

# Context manager pour ressources
with open("file.txt") as f:
    data = f.read()
```

### Pièges
- `except:` nu → toujours spécifier l'exception
- `requirements.txt` sans versions → `pip freeze > requirements.txt`
- Mutable default args : `def f(x=[])` → `def f(x=None)`
- `pip install` sans venv → pollue le système
