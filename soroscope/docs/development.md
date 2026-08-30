# Development Guide

This guide covers setting up your environment and the development standards we follow at SoroScope.

## 🚀 Getting Started

### Prerequisites
- **Rust** (latest stable)
- **Node.js** (>= 18)
- **Soroban CLI**

### Monorepo Setup
1.  **Fork** the repository and clone it locally.
2.  **Rust Core**: Build the backend.
    ```bash
    cargo build -p soroscope-core
    ```
3.  **Web Dashboard**: Install frontend dependencies.
    ```bash
    cd web
    npm install
    ```
4.  **Contracts**: Compile the sample contracts.
    ```bash
    cargo build --target wasm32-unknown-unknown --release
    ```

## 🛠️ Development Standards

### Rust Code
- **Formatting**: Always run `cargo fmt` before committing.
- **Linting**: Run `cargo clippy` to check for common mistakes.
- **Tests**: Ensure all tests pass with `cargo test`.

### Frontend (Next.js)
- **Styling**: Use Tailwind CSS for consistency.
- **Linting**: Run `npm run lint` within the `/web` directory.
- **Components**: Keep components modular and placed in `/web/components`.
- **State Persistence**: See [Frontend Persistence](./FRONTEND_PERSISTENCE.md) for details on how analysis results are persisted across page refreshes.

### Contracts
- Use **Soroban SDK v22.0.0** or higher.
- Avoid deprecated methods like `register_contract` (use `register` instead).
