# ADR 002: Trait-based module isolation

**Status:** Accepted

**Context:** The spec requires "every module exposes a trait, not a concrete type. Modules are testable in isolation against their trait with mocks."

**Decision:** Every major module (Brain, Tool, Sandbox, Router, Memory, Engine, Orchestrator, GatewayTransport, etc.) is defined as a trait. Concrete implementations live in their own modules. Tests use mock implementations of the traits.

**Consequences:** Increased boilerplate (trait + impl). Enables true isolation testing. Prevents tight coupling.
