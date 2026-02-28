# Next.js Conventions (App Router)

## App Router Rules
- All routes in `app/` directory — never mix with `pages/`
- Route handlers: `app/api/.../route.ts` — export GET/POST/etc
- Server components are the default — only add `"use client"` when needed (interactivity, hooks, browser APIs)
- Layouts: `app/layout.tsx` (root) + `app/(group)/layout.tsx` for nested layouts

## Server Actions
```ts
"use server"
export async function submitForm(formData: FormData) {
  // validate → db → revalidatePath
}
```
- Always validate with zod before touching DB
- Use `revalidatePath` / `revalidateTag` for cache invalidation

## Data Fetching
- Server components: fetch directly, use `cache()` for deduplication
- ISR: `fetch(url, { next: { revalidate: 60 } })`
- Dynamic: `export const dynamic = "force-dynamic"`
- Static: default (no export)

## File Conventions
- `page.tsx` — route segment
- `layout.tsx` — persistent shell
- `loading.tsx` — Suspense fallback
- `error.tsx` — error boundary
- `not-found.tsx` — 404 handler

## Performance
- Images: always `next/image` with explicit width/height
- Fonts: `next/font` — never @import from Google Fonts directly
- Metadata: export `metadata` object or `generateMetadata()` — never `<head>` tags
