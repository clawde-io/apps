// SPDX-License-Identifier: MIT
//! Built-in stack templates for Builder Mode.
//!
//! Each template ships with working boilerplate that can be opened and
//! iterated on immediately after scaffolding.  Templates are intentionally
//! minimal — just enough to be runnable — leaving room for the AI session
//! to add real features.

use super::model::{StackTemplate, TemplateFile};

/// Return all built-in templates.
///
/// Called by `handlers::builder_list_templates` and by `handlers::builder_create_session`
/// to look up the requested stack before writing files.
pub fn all_templates() -> Vec<StackTemplate> {
    vec![
        react_vite(),
        nextjs_tailwind(),
        express_prisma(),
        rust_axum(),
        flutter_riverpod(),
    ]
}

/// Find a template by its machine-readable `name`.
pub fn find_template(name: &str) -> Option<StackTemplate> {
    all_templates().into_iter().find(|t| t.name == name)
}

// ─── react-vite ─────────────────────────────────────────────────────────────

fn react_vite() -> StackTemplate {
    StackTemplate {
        name: "react-vite".into(),
        description: "React 18 + Vite 5 + TypeScript — fast SPA with HMR".into(),
        files: vec![
            TemplateFile {
                path: "index.html".into(),
                content: r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <link rel="icon" type="image/svg+xml" href="/vite.svg" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>My App</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
"#
                .into(),
            },
            TemplateFile {
                path: "vite.config.ts".into(),
                content: r#"import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    strictPort: true,
  },
});
"#
                .into(),
            },
            TemplateFile {
                path: "tsconfig.json".into(),
                content: r#"{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
"#
                .into(),
            },
            TemplateFile {
                path: "package.json".into(),
                content: r#"{
  "name": "my-app",
  "private": true,
  "version": "0.0.1",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "lint": "eslint . --ext ts,tsx --report-unused-disable-directives --max-warnings 0"
  },
  "dependencies": {
    "react": "^18.3.1",
    "react-dom": "^18.3.1"
  },
  "devDependencies": {
    "@types/react": "^18.3.3",
    "@types/react-dom": "^18.3.0",
    "@vitejs/plugin-react": "^4.3.1",
    "typescript": "^5.5.3",
    "vite": "^5.3.4"
  }
}
"#
                .into(),
            },
            TemplateFile {
                path: "src/main.tsx".into(),
                content: r#"import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App.tsx';
import './index.css';

const root = document.getElementById('root');
if (!root) throw new Error('Root element not found');

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
"#
                .into(),
            },
            TemplateFile {
                path: "src/App.tsx".into(),
                content: r#"import { useState } from 'react';

function App() {
  const [count, setCount] = useState(0);

  return (
    <main style={{ fontFamily: 'system-ui', maxWidth: 640, margin: '0 auto', padding: '2rem' }}>
      <h1>My App</h1>
      <p>Edit <code>src/App.tsx</code> to get started.</p>
      <button onClick={() => setCount((c) => c + 1)}>
        Count: {count}
      </button>
    </main>
  );
}

export default App;
"#
                .into(),
            },
            TemplateFile {
                path: "src/index.css".into(),
                content: r#":root {
  font-family: system-ui, sans-serif;
  line-height: 1.5;
  font-weight: 400;
  color-scheme: light dark;
  color: rgba(255, 255, 255, 0.87);
  background-color: #242424;
}

body {
  margin: 0;
  min-width: 320px;
  min-height: 100vh;
}

button {
  border-radius: 8px;
  border: 1px solid transparent;
  padding: 0.6em 1.2em;
  font-size: 1em;
  font-weight: 500;
  font-family: inherit;
  background-color: #1a1a1a;
  cursor: pointer;
  transition: border-color 0.25s;
}
button:hover {
  border-color: #646cff;
}
"#
                .into(),
            },
        ],
    }
}

// ─── nextjs-tailwind ─────────────────────────────────────────────────────────

fn nextjs_tailwind() -> StackTemplate {
    StackTemplate {
        name: "nextjs-tailwind".into(),
        description: "Next.js 14 App Router + Tailwind CSS + TypeScript".into(),
        files: vec![
            TemplateFile {
                path: "package.json".into(),
                content: r#"{
  "name": "my-nextjs-app",
  "version": "0.1.0",
  "private": true,
  "scripts": {
    "dev": "next dev",
    "build": "next build",
    "start": "next start",
    "lint": "next lint"
  },
  "dependencies": {
    "next": "^14.2.5",
    "react": "^18.3.1",
    "react-dom": "^18.3.1"
  },
  "devDependencies": {
    "@types/node": "^20",
    "@types/react": "^18",
    "@types/react-dom": "^18",
    "autoprefixer": "^10.4.19",
    "postcss": "^8.4.39",
    "tailwindcss": "^3.4.6",
    "typescript": "^5.5.3"
  }
}
"#
                .into(),
            },
            TemplateFile {
                path: "next.config.mjs".into(),
                content: r#"/** @type {import('next').NextConfig} */
const nextConfig = {};

export default nextConfig;
"#
                .into(),
            },
            TemplateFile {
                path: "tailwind.config.ts".into(),
                content: r#"import type { Config } from 'tailwindcss';

const config: Config = {
  content: [
    './src/pages/**/*.{js,ts,jsx,tsx,mdx}',
    './src/components/**/*.{js,ts,jsx,tsx,mdx}',
    './src/app/**/*.{js,ts,jsx,tsx,mdx}',
  ],
  theme: {
    extend: {},
  },
  plugins: [],
};

export default config;
"#
                .into(),
            },
            TemplateFile {
                path: "src/app/layout.tsx".into(),
                content: r#"import type { Metadata } from 'next';
import './globals.css';

export const metadata: Metadata = {
  title: 'My App',
  description: 'Built with Next.js + Tailwind',
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
"#
                .into(),
            },
            TemplateFile {
                path: "src/app/page.tsx".into(),
                content: r#"export default function Home() {
  return (
    <main className="flex min-h-screen flex-col items-center justify-center p-24">
      <h1 className="text-4xl font-bold tracking-tight text-gray-900">
        My App
      </h1>
      <p className="mt-4 text-lg text-gray-600">
        Edit <code className="font-mono bg-gray-100 px-1 rounded">src/app/page.tsx</code> to get started.
      </p>
    </main>
  );
}
"#
                .into(),
            },
            TemplateFile {
                path: "src/app/globals.css".into(),
                content: r#"@tailwind base;
@tailwind components;
@tailwind utilities;
"#
                .into(),
            },
        ],
    }
}

// ─── express-prisma ───────────────────────────────────────────────────────────

fn express_prisma() -> StackTemplate {
    StackTemplate {
        name: "express-prisma".into(),
        description: "Express 4 + Prisma ORM + TypeScript REST API".into(),
        files: vec![
            TemplateFile {
                path: "package.json".into(),
                content: r#"{
  "name": "my-api",
  "version": "0.1.0",
  "private": true,
  "scripts": {
    "dev": "ts-node-dev --respawn --transpile-only src/index.ts",
    "build": "tsc",
    "start": "node dist/index.js",
    "db:migrate": "prisma migrate dev",
    "db:generate": "prisma generate"
  },
  "dependencies": {
    "@prisma/client": "^5.16.2",
    "express": "^4.19.2",
    "cors": "^2.8.5"
  },
  "devDependencies": {
    "@types/cors": "^2.8.17",
    "@types/express": "^4.17.21",
    "@types/node": "^20",
    "prisma": "^5.16.2",
    "ts-node-dev": "^2.0.0",
    "typescript": "^5.5.3"
  }
}
"#
                .into(),
            },
            TemplateFile {
                path: "tsconfig.json".into(),
                content: r#"{
  "compilerOptions": {
    "target": "ES2020",
    "module": "commonjs",
    "lib": ["ES2020"],
    "outDir": "dist",
    "rootDir": "src",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true
  },
  "include": ["src"],
  "exclude": ["node_modules", "dist"]
}
"#
                .into(),
            },
            TemplateFile {
                path: "prisma/schema.prisma".into(),
                content: r#"// This is your Prisma schema file.
// Learn more: https://pris.ly/d/prisma-schema

generator client {
  provider = "prisma-client-js"
}

datasource db {
  provider = "sqlite"
  url      = env("DATABASE_URL")
}

model User {
  id        Int      @id @default(autoincrement())
  email     String   @unique
  name      String?
  createdAt DateTime @default(now())
  updatedAt DateTime @updatedAt
}
"#
                .into(),
            },
            TemplateFile {
                path: "src/index.ts".into(),
                content: r#"import express from 'express';
import cors from 'cors';
import { PrismaClient } from '@prisma/client';

const app = express();
const prisma = new PrismaClient();
const PORT = process.env.PORT ?? 3000;

app.use(cors());
app.use(express.json());

// Health check
app.get('/health', (_req, res) => {
  res.json({ status: 'ok', timestamp: new Date().toISOString() });
});

// List users
app.get('/users', async (_req, res) => {
  try {
    const users = await prisma.user.findMany();
    res.json(users);
  } catch (err) {
    res.status(500).json({ error: 'Internal server error' });
  }
});

// Create user
app.post('/users', async (req, res) => {
  const { email, name } = req.body as { email?: string; name?: string };
  if (!email) {
    return res.status(400).json({ error: 'email is required' });
  }
  try {
    const user = await prisma.user.create({ data: { email, name } });
    res.status(201).json(user);
  } catch (err) {
    res.status(409).json({ error: 'Email already exists' });
  }
});

app.listen(PORT, () => {
  console.log(`API running on http://localhost:${PORT}`);
});

process.on('SIGTERM', async () => {
  await prisma.$disconnect();
  process.exit(0);
});
"#
                .into(),
            },
            TemplateFile {
                path: ".env".into(),
                content: r#"DATABASE_URL="file:./dev.db"
PORT=3000
"#
                .into(),
            },
        ],
    }
}

// ─── rust-axum ────────────────────────────────────────────────────────────────

fn rust_axum() -> StackTemplate {
    StackTemplate {
        name: "rust-axum".into(),
        description: "Rust + Axum 0.7 + Tokio — async HTTP API".into(),
        files: vec![
            TemplateFile {
                path: "Cargo.toml".into(),
                content: r#"[package]
name = "my-api"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "server"
path = "src/main.rs"

[dependencies]
axum = "0.7"
tokio = { version = "1", features = ["full"] }
tower = { version = "0.5", features = ["util"] }
tower-http = { version = "0.5", features = ["cors", "trace"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1"
"#
                .into(),
            },
            TemplateFile {
                path: "src/main.rs".into(),
                content: r#"// SPDX-License-Identifier: MIT
use axum::{
    routing::{get, post},
    Router,
};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod routes;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "my_api=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new()
        .route("/health", get(routes::health::handler))
        .route("/api/items", get(routes::items::list))
        .route("/api/items", post(routes::items::create))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
"#
                .into(),
            },
            TemplateFile {
                path: "src/routes/mod.rs".into(),
                content: r#"// SPDX-License-Identifier: MIT
pub mod health;
pub mod items;
"#
                .into(),
            },
            TemplateFile {
                path: "src/routes/health.rs".into(),
                content: r#"// SPDX-License-Identifier: MIT
use axum::Json;
use serde_json::{json, Value};

pub async fn handler() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "timestamp": chrono_now(),
    }))
}

fn chrono_now() -> String {
    // Use std time to avoid pulling in chrono for this simple template.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}", now)
}
"#
                .into(),
            },
        ],
    }
}

// ─── flutter-riverpod ─────────────────────────────────────────────────────────

fn flutter_riverpod() -> StackTemplate {
    StackTemplate {
        name: "flutter-riverpod".into(),
        description: "Flutter + Riverpod 2 — cross-platform app with state management".into(),
        files: vec![
            TemplateFile {
                path: "pubspec.yaml".into(),
                content: r#"name: my_app
description: A new Flutter application.
publish_to: 'none'
version: 0.1.0

environment:
  sdk: '>=3.3.0 <4.0.0'
  flutter: '>=3.19.0'

dependencies:
  flutter:
    sdk: flutter
  flutter_riverpod: ^2.5.1
  riverpod_annotation: ^2.3.5
  go_router: ^14.2.7
  cupertino_icons: ^1.0.6

dev_dependencies:
  flutter_test:
    sdk: flutter
  flutter_lints: ^4.0.0
  build_runner: ^2.4.11
  riverpod_generator: ^2.4.3

flutter:
  uses-material-design: true
"#
                .into(),
            },
            TemplateFile {
                path: "lib/main.dart".into(),
                content: r#"import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'app.dart';

void main() {
  runApp(
    const ProviderScope(
      child: MyApp(),
    ),
  );
}
"#
                .into(),
            },
            TemplateFile {
                path: "lib/app.dart".into(),
                content: r#"import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Simple counter provider to demonstrate Riverpod state management.
final counterProvider = StateProvider<int>((ref) => 0);

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'My App',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: Colors.deepPurple),
        useMaterial3: true,
      ),
      home: const _HomeScreen(),
    );
  }
}

class _HomeScreen extends ConsumerWidget {
  const _HomeScreen();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final count = ref.watch(counterProvider);

    return Scaffold(
      appBar: AppBar(
        backgroundColor: Theme.of(context).colorScheme.inversePrimary,
        title: const Text('My App'),
      ),
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Text('You have pushed the button this many times:'),
            Text(
              '$count',
              style: Theme.of(context).textTheme.headlineMedium,
            ),
          ],
        ),
      ),
      floatingActionButton: FloatingActionButton(
        onPressed: () => ref.read(counterProvider.notifier).state++,
        tooltip: 'Increment',
        child: const Icon(Icons.add),
      ),
    );
  }
}
"#
                .into(),
            },
        ],
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_templates_returns_five() {
        let templates = all_templates();
        assert_eq!(templates.len(), 5);
    }

    #[test]
    fn find_template_returns_correct_stack() {
        let tpl = find_template("react-vite").expect("react-vite should exist");
        assert_eq!(tpl.name, "react-vite");
        assert!(!tpl.files.is_empty());
    }

    #[test]
    fn find_template_returns_none_for_unknown() {
        assert!(find_template("does-not-exist").is_none());
    }

    #[test]
    fn react_vite_template_has_package_json() {
        let tpl = find_template("react-vite").unwrap();
        let paths: Vec<&str> = tpl.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"package.json"));
        assert!(paths.contains(&"src/App.tsx"));
    }

    #[test]
    fn rust_axum_template_has_cargo_toml() {
        let tpl = find_template("rust-axum").unwrap();
        let paths: Vec<&str> = tpl.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"Cargo.toml"));
        assert!(paths.contains(&"src/main.rs"));
    }

    #[test]
    fn flutter_riverpod_template_has_pubspec() {
        let tpl = find_template("flutter-riverpod").unwrap();
        let paths: Vec<&str> = tpl.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"pubspec.yaml"));
        assert!(paths.contains(&"lib/main.dart"));
    }

    #[test]
    fn all_templates_have_non_empty_descriptions() {
        for tpl in all_templates() {
            assert!(!tpl.description.is_empty(), "template '{}' has empty description", tpl.name);
            assert!(!tpl.files.is_empty(), "template '{}' has no files", tpl.name);
        }
    }
}
