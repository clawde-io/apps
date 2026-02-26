# Arena Mode

Arena Mode runs the same prompt through multiple AI providers simultaneously and presents the results side-by-side so you can pick the best answer or merge outputs.

## How it works

1. You send a prompt in Arena Mode (toggle in session settings or via `clawd session --arena`)
2. The daemon dispatches the request to 2-4 configured providers in parallel
3. Each response streams into a separate panel in the desktop app
4. You choose a winner, merge hunks, or discard all and retry

## Use cases

- Compare code quality between Claude, Codex, and GPT-4 on the same refactor
- Evaluate model regression when switching provider versions
- Get consensus on architectural decisions — agree = confident, disagree = needs human input

## Status

Planned for Sprint GG. Requires multi-session dispatch in the daemon + arena layout in the desktop app.

## Related

- [Providers](Providers.md)
- [Session Manager](Session-Manager.md)
- [Model Intelligence](Model-Intelligence.md)
