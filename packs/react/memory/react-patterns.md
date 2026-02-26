# React Conventions

## Component Rules
- Functional components only — no class components
- One component per file; file name matches component name
- Props interface defined above component: `interface FooProps { ... }`
- Default export the component; named exports for utilities

## Hook Rules
- Custom hooks start with `use`: `useAuth`, `useDebounce`
- Never call hooks conditionally
- Prefer `useState` + `useReducer` over prop drilling > 2 levels
- Use `useCallback` for handlers passed as props; `useMemo` for expensive computations only

## State Management
- Local state: `useState`
- Server state: React Query / SWR
- Global state: Context API (small) or Zustand (large)
- Avoid Redux unless codebase already uses it

## Testing
- Tests colocated: `Foo.tsx` → `Foo.test.tsx`
- Use React Testing Library — never test implementation details
- Query priority: `getByRole` > `getByLabelText` > `getByText` > `getByTestId`
- Mock at the module boundary, not the component level

## File Structure
```
src/
  components/    # Shared UI components
  features/      # Feature modules (each has own components/, hooks/, types/)
  hooks/         # Shared custom hooks
  lib/           # Utilities, API clients
  types/         # Shared TypeScript types
```
