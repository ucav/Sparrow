# Skill: Security Audit

**Trigger:** security, vulnerability, secret, CVE, audit security, check for secrets

**Description:** Security audit: secrets in code, injection vectors, vulnerable dependencies.

## Body
When auditing security:
1. SCAN for secrets: API keys, tokens, passwords in source files. Use search tool.
2. CHECK dependencies: cargo audit, npm audit, pip-audit. Report vulnerabilities.
3. INJECTION vectors: SQL, command injection, XSS. Review user input handling.
4. ACCESS control: Are sensitive operations properly gated?
5. Report each finding with severity (critical/high/medium/low) and remediation.
6. NEVER store or expose found secrets. Redact before reporting.
