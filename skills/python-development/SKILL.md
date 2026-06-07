# Skill: Python Development
**Trigger:** python code, pip, venv, write python
**Description:** Python development best practices — virtualenv, testing, typing, packaging.

## Body
1. Virtualenv: python3 -m venv .venv && source .venv/bin/activate
2. Dependencies: pip install, requirements.txt or pyproject.toml
3. Format: black . && isort .
4. Lint: ruff check . or flake8
5. Type check: mypy .
6. Test: pytest -xvs
7. Use type hints (def foo(x: int) -> str)
8. Use dataclasses or Pydantic for data models
9. Error handling: specific exceptions, not bare except
10. Packaging: pyproject.toml with setuptools or poetry
