# TypeScript Conventions

## Strict Mode
Always use `strict: true` in `tsconfig.json`. This enables:
- `strictNullChecks` — no implicit `undefined`/`null`
- `noImplicitAny` — all parameters typed explicitly
- `strictFunctionTypes` — covariant/contravariant function type checks

## Type Patterns
- Prefer `interface` for object shapes that may be extended; `type` for unions/intersections/aliases
- Use `unknown` instead of `any` at boundaries — then narrow with type guards
- `as const` for literal type inference: `const STATUS = { ACTIVE: "active" } as const`
- Discriminated unions: `type Result<T> = { ok: true; data: T } | { ok: false; error: string }`

## Schema Validation (zod)
```ts
import { z } from "zod"
const UserSchema = z.object({ id: z.string().uuid(), email: z.string().email() })
type User = z.infer<typeof UserSchema>  // derive type from schema
```
- Parse at system boundaries (API responses, env vars, form inputs)
- Never use `z.any()` — always model the actual shape

## Naming
- Types/Interfaces: `PascalCase`
- Variables/functions: `camelCase`
- Constants: `SCREAMING_SNAKE_CASE` for module-level; `camelCase` for local
- No `I` prefix on interfaces (no `IUser`)

## Module Boundaries
- Barrel exports (`index.ts`) only for feature public API
- Internal modules import directly — never barrel-import within a feature
- Avoid circular imports — use dependency injection or event buses to break cycles
