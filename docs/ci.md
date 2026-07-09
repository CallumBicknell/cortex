# CI / CD

Cortex uses **GitHub Actions** for continuous integration and releases, plus
local scripts that mirror CI for pre-push confidence.

## Workflows

| Workflow | Trigger | What it does |
|----------|---------|----------------|
| [`ci.yml`](../.github/workflows/ci.yml) | PR / push | fmt, clippy, tests, evals, smoke, cargo-deny, Python SDK, release binary artifact |
| [`release.yml`](../.github/workflows/release.yml) | tag `v*` | Multi-OS release binaries + GitHub Release |
| [`docker.yml`](../.github/workflows/docker.yml) | main / tags (PRs only if Docker files change) | Build & push image to GHCR |

## Jobs (CI)

1. **lint** — `cargo fmt --check`, `cargo clippy -D warnings`
2. **test** — `cargo test --workspace --all-targets`
3. **eval** — `cortex eval run` (offline mock fixtures in `evals/`)
4. **smoke** — `./scripts/smoke_agent.sh` (mock `cortex run`)
5. **deny** — `cargo deny check` (advisories, licenses, sources)
6. **python** — pytest for `sdks/python` (3.10 + 3.12)
7. **build-release** — `cargo build --release -p cortex-cli` + artifact

## Local parity

```bash
# Full suite (recommended before PR)
./scripts/ci_local.sh

# Or via make / just
make ci
just ci

# Individual gates
make lint test eval smoke python-test
```

Install optional tools:

```bash
cargo install cargo-deny --locked
```

## Releases

1. Ensure CI is green on `main`.
2. Tag a semver release:

```bash
git tag -a v0.2.0 -m "v0.2.0"
git push origin v0.2.0
```

3. **Release** workflow builds binaries for:
   - Linux x86_64
   - macOS aarch64 / x86_64
   - Windows x86_64
   and attaches them to a GitHub Release with notes.

4. **Docker** (optional): image at `ghcr.io/<owner>/<repo>` on main/tags.

```bash
# Local build (BuildKit + layer/cache mounts; needs Docker Buildx)
DOCKER_BUILDKIT=1 docker build -t cortex:local .
docker run --rm -p 8080:8080 -v "$PWD:/workspace" -w /workspace \
  cortex:local serve --bind 0.0.0.0:8080
```

### Docker CI performance

The image build uses:

1. **cargo-chef** — dependency graph cooked in a separate layer; app code changes
   do not recompile all crates.io deps from scratch.
2. **BuildKit cache mounts** — registry/git/`target` directories reuse work across
   builds when GHA cache hits.
3. **`.dockerignore`** — excludes `target/`, `.git`, docs, SDK, etc. so the
   build context stays small.
4. **PR path filters** — Docker job runs on PRs only when `Dockerfile` /
   `.dockerignore` / the workflow change. Normal Rust PRs rely on `ci.yml`.
5. **GHA cache** (`type=gha,mode=max`) shared as scope `cortex-docker`.

## Dependabot

[`.github/dependabot.yml`](../.github/dependabot.yml) opens weekly PRs for:

- GitHub Actions
- Cargo
- Python SDK (`sdks/python`)
- Docker base images

## Pre-commit

Local hook (`.git/hooks/pre-commit`) and [`.pre-commit-config.yaml`](../.pre-commit-config.yaml):

- trailing whitespace
- `cargo fmt --check`
- `cargo clippy -D warnings` (can be slow; CI is the source of truth)

```bash
pre-commit install   # if using the pre-commit framework
```

## Lockfile

Root `Cargo.lock` is **committed** for reproducible CI (binary workspace).

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| fmt fails | `cargo fmt --all` |
| clippy fails | `cargo clippy --workspace --fix` then fix |
| deny fails | check `deny.toml` allow lists / advisories |
| python fails | `cd sdks/python && python -m venv .venv && pip install -e ".[dev]"` |
| eval fails | run `cargo run -p cortex-cli -- eval run` locally |
