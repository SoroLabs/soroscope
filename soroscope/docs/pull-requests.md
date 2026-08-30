# How to Open a Pull Request

We follow the standard **Fork & Pull** workflow. If you're new to this, here is the exact step-by-step:

### 1. Fork & Clone
1.  Click the **Fork** button at the top of the [Soroban Scope repository](https://github.com/SoroLabs/soroscope).
2.  Clone your fork locally:
    ```bash
    git clone https://github.com/YOUR_USERNAME/soroscope.git
    cd soroscope
    ```
3.  Add the original repository as an `upstream` remote:
    ```bash
    git remote add upstream https://github.com/SoroLabs/soroscope.git
    ```

### 2. Create a Branch
Always work on a new branch, never on `main`:
```bash
git checkout -b feat/your-feature-name
```

### 3. Make Changes & Verify
Implement your changes and run the local verification commands (see the [Development Guide](./development.md) for details):
```bash
# For Rust/Contracts
cargo fmt
cargo test

# For Web
cd web
npm run lint
```

### 4. Push & Open PR
1.  Commit and push your branch:
    ```bash
    git add .
    git commit -m "feat: descriptive message"
    git push origin feat/your-feature-name
    ```
2.  Go to the [Soroban Scope PR page](https://github.com/SoroLabs/soroscope/pulls).
3.  You should see a yellow banner saying **"Compare & pull request"**. Click it!
4.  Write a clear description of your changes and submit.

### 5. Address Feedback
A maintainer will review your code. If changes are requested, simply commit them to your branch and push again—the PR will update automatically.
