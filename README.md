# LogSmart - COMP2003-2025-2026

Introducing LogSmart, the template-based digital logging system initially designed for the food service industry. Replace your paper logs with an instant, secure, and eco-friendly solution that optimizes your daily operations. Featuring a custom WYSIWYG template designer, AI-assisted template generation, scheduled logs, built-in report generation and simple user administration tools.

![](https://github.com/LogSmartDev/LogSmart/blob/main/Design%20Documents/template_designer.png)
## 🔗 Links

- **Production:** [https://logsmart.app/](https://logsmart.app/)
- **API:** [https://logsmart.app/api/](https://logsmart.app/api/)
- **API Docs:** [https://api.logsmart.app/swagger-ui](https://api.logsmart.app/swagger-ui)
- **Direct API:** [https://api.logsmart.app/](https://api.logsmart.app/)
- **Repository:** [https://github.com/LogSmartDev/LogSmart](https://github.com/LogSmartDev/LogSmart)

## 🛠️ Tech Stack

### Front-end
- **Framework:** SvelteKit
- **Styling:** Tailwind CSS
- **Runtime:** Bun
- **Hosting:** Cloudflare Pages

### Back-end
- **Language:** Rust
- **Framework:** Axum
- **Databases:**
  - PostgreSQL (authentication, security logs, users)
  - MongoDB (templates, customer logs)
- **Hosting:** Oracle VPS (ARM64) via Docker Swarm

## 🚀 Quick Start

### Prerequisites
- **Front-end:** [Bun](https://bun.sh/)
- **Back-end:** 
  - [Nix](https://nixos.org/) (for development environment)
  - [Docker](https://www.docker.com/) & [Docker Compose](https://docs.docker.com/compose/) (for local databases)

### Local Development

**Front-end:**
```bash
cd SourceCode/front-end/logsmart
bun install
bun run dev
```

**Back-end:**
```bash
cd SourceCode/back-end

# Start backend and databases (PostgreSQL and MongoDB)
docker compose up -d
```

## 📋 Development Workflow

### Git Branching Strategy
- `main` - Development branch for integration testing
- `prod` - Production branch (triggers automatic deployment)
- `feature/*` - Feature branches

### Commit Message Convention
Format: `Topic: Message`

Examples:
- `Front-End: Add dashboard component`
- `Back-End: Implement user authentication`
- `CI/CD: Update deployment workflow`

### Branch Management Rules
1. Always work on a feature branch
2. Merge feature branch to `main` for integration testing
3. Only merge `main` to `prod` after complete testing
4. Production deployments are triggered automatically on push to `prod`

### Common Git Commands

**Setup git graph alias:**
```bash
git config --global alias.graph "log --all --decorate --oneline --graph"
```

**Update feature branch with latest main:**
```bash
git stash                      # Stash uncommitted changes
git fetch                      # Fetch from origin
git switch main                # Switch to main
git pull                       # Pull latest changes
git switch my-feature-branch   # Switch back to feature branch
git rebase main                # Rebase onto updated main
git push --force-with-lease    # Force push rebased branch
git stash pop                  # Restore uncommitted changes
```

**Merge feature branch into main:**
```bash
git fetch                      # Fetch from origin
git switch main                # Switch to main
git pull                       # Update main
git merge my-feature-branch    # Merge feature branch
git push                       # Push to origin
```

**Deploy to production:**
```bash
git fetch                      # Fetch from origin
git switch prod                # Switch to prod
git pull                       # Update prod
git merge main                 # Merge main into prod
git push                       # Push to origin (triggers deployment)
```

## 🔄 CI/CD & Deployment

### Automated Pull Request Workflow
When a PR is successfully merged into `main`, a GitHub Action automatically:
1. Creates a new pull request from `main` to `prod` (if one doesn't exist)
2. OR updates the existing `main` → `prod` PR with a comment about the new changes

This ensures that all changes are reviewed before being deployed to production.

### Automatic Deployment
Push to the `prod` branch triggers automatic deployment:
- **Front-end:** Deploys to Cloudflare Pages via Wrangler
- **Back-end:** Builds Docker image with Nix and deploys to Oracle VPS via Docker Swarm

The backend is deployed as a Docker service alongside PostgreSQL and MongoDB containers, all managed by Docker Swarm for high availability and automatic rollback on failure.

### Manual Deployment

**Front-end:**
```bash
cd SourceCode/front-end/logsmart
bun run deploy
```

**Back-end:**
```bash
# Build locally for ARM64
cd SourceCode/back-end
nix build .#aarch64-linux

# Build Docker image
nix build .#docker-image-aarch64

# The image can be loaded and deployed to Docker Swarm
docker load < result
docker service update logsmart_backend --image <image-name>
```

### Required GitHub Secrets
**Front-end:**
- `CLOUDFLARE_API_TOKEN`
- `CLOUDFLARE_ACCOUNT_ID`

**Back-end:**
- `DEPLOY_HOST` - Server IP/hostname
- `DEPLOY_USER` - SSH username
- `DEPLOY_SSH_KEY` - Private SSH key for authentication
- `DEPLOY_PORT` - SSH port (usually 22)
- `DEPLOY_PATH` - Target directory on server

### Monitoring Logs
View real-time logs from Cloudflare Pages:
```bash
cd SourceCode/front-end/logsmart
bun run logs
```

Or using Wrangler directly:
```bash
wrangler pages deployment tail --project-name=logsmart
```

## 📁 Project Structure
```
SourceCode/
├── front-end/
│   └── logsmart/          # SvelteKit application
│       ├── src/
│       │   ├── routes/    # SvelteKit routes
│       │   └── lib/       # Shared components and utilities
│       └── package.json
└── back-end/              # Rust API server
    ├── src/
    │   ├── main.rs        # Application entry point
    │   ├── handlers/      # API route handlers
    │   ├── services/      # Business logic services
    │   ├── auth.rs        # Authentication logic
    │   ├── db.rs          # PostgreSQL database operations
    │   ├── logs_db.rs     # MongoDB log operations
    │   ├── middleware.rs  # Request middleware
    │   ├── rate_limit.rs  # Rate limiting
    │   └── ...
    ├── tests/             # Integration and unit tests
    ├── migrations/        # PostgreSQL migrations
    ├── Cargo.toml
    ├── docker-compose.yml # Local development databases
    └── flake.nix          # Nix development environment
```

## 🧪 Testing

### Back-end Tests
The back-end includes comprehensive test suites:

```bash
cd SourceCode/back-end
nix develop

# Run all tests
cargo test

# Run specific test files
cargo test --test http_api_tests
cargo test --test auth_tests
cargo test --test rate_limit_tests

# Run tests with output
cargo test -- --nocapture
```

Test categories include:
- API integration tests
- Authentication tests
- Database tests
- Handler tests
- Rate limiting tests
- Middleware tests
- Property-based tests
- Performance tests

### Front-end Tests
End-to-end tests using Playwright:

```bash
cd SourceCode/front-end/logsmart
bun install
bun run test
```

### CI/CD Testing
All tests are automatically run on pull requests via GitHub Actions. The test suite workflow includes:
- Building the backend Docker image
- Starting PostgreSQL and MongoDB via Docker Compose
- Running Playwright end-to-end tests
- Code formatting and linting checks

## 🐛 Troubleshooting

### Swagger UI blank on custom domain
If Swagger UI loads on `*.pages.dev` but not on your custom domain:
1. Go to Cloudflare Dashboard → Speed → Optimization
2. Disable **Rocket Loader**
3. Purge cache (Caching → Purge Everything)

### Build fails on GitHub Actions
- Check that all required secrets are set correctly
- Verify Nix flake builds locally: `nix build .#aarch64-linux`
- Review workflow logs in GitHub Actions tab

### Database Connection Issues
If you encounter database connection errors during local development:
1. Ensure Docker Compose services are running: `docker-compose ps`
2. Check database logs: `docker-compose logs postgres` or `docker-compose logs mongodb`
3. Verify your `.envrc` file has correct database connection strings
4. Restart databases: `docker-compose restart postgres mongodb`

### Migration Issues
If database migrations fail:
```bash
# Check migration status
sqlx migrate info

# Revert last migration
sqlx migrate revert

# Run migrations again
sqlx migrate run
```
