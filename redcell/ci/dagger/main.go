package main

import (
	"context"
	"fmt"
	"os"
	"strings"
	"sync"
	"time"

	"dagger/redcell/internal/dagger"
)

const (
	defaultRegistry = "ghcr.io/redcell"
	rustImage       = "rust:1.95-bookworm"
	flyctlImage     = "flyio/flyctl:latest"
)

type Redcell struct{}

func withoutGit(src *dagger.Directory) *dagger.Directory {
	return src.WithoutDirectory(".git")
}

const (
	cargoRegistryCache = "cargo-registry"
	cargoGitCache      = "cargo-git"
	cargoDebugCache    = "cargo-target-debug"
	cargoReleaseCache  = "cargo-target-release"
)

// rustToolchainBase returns a Rust container with fmt and clippy installed.
func rustToolchainBase() *dagger.Container {
	return dag.Container().
		From(rustImage).
		WithExec([]string{"rustup", "component", "add", "rustfmt", "clippy"})
}

// rustProject mounts source and Cargo caches on top of the toolchain base.
func rustProject(src *dagger.Directory, targetTag string) *dagger.Container {
	cargoRegistry := dag.CacheVolume(cargoRegistryCache)
	cargoGit := dag.CacheVolume(cargoGitCache)
	targetCache := dag.CacheVolume(targetTag)

	return rustToolchainBase().
		WithMountedDirectory("/src", src).
		WithWorkdir("/src").
		WithMountedCache("/usr/local/cargo/registry", cargoRegistry).
		WithMountedCache("/usr/local/cargo/git", cargoGit).
		WithMountedCache("/src/target", targetCache)
}

// Lint runs cargo fmt --check, cargo clippy, and cargo check.
func (m *Redcell) Lint(ctx context.Context, src *dagger.Directory) error {
	src = withoutGit(src)
	builder := rustProject(src, cargoDebugCache)

	fmtCheck := builder.WithExec([]string{"cargo", "fmt", "--check"})
	clippyCheck := builder.WithExec([]string{"cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"})
	checkCheck := builder.WithExec([]string{"cargo", "check", "--workspace"})

	if _, err := fmtCheck.Sync(ctx); err != nil {
		return fmt.Errorf("cargo fmt: %w", err)
	}
	if _, err := clippyCheck.Sync(ctx); err != nil {
		return fmt.Errorf("cargo clippy: %w", err)
	}
	if _, err := checkCheck.Sync(ctx); err != nil {
		return fmt.Errorf("cargo check: %w", err)
	}
	return nil
}

// Test runs cargo test --workspace.
func (m *Redcell) Test(ctx context.Context, src *dagger.Directory) error {
	src = withoutGit(src)
	_, err := rustProject(src, cargoDebugCache).
		WithEnvVariable("CARGO_BUILD_JOBS", "2").
		WithEnvVariable("CARGO_INCREMENTAL", "0").
		WithExec([]string{"cargo", "test", "--workspace"}).
		Sync(ctx)
	return err
}

// Build compiles the Redcell release binary and returns the Docker image.
func (m *Redcell) Build(ctx context.Context, src *dagger.Directory) (*dagger.Container, error) {
	src = withoutGit(src)

	builder := rustProject(src, cargoReleaseCache).
		WithExec([]string{"cargo", "build", "--release"}).
		WithExec([]string{"cp", "/src/target/release/redcell", "/tmp/redcell"})

	if _, err := builder.Sync(ctx); err != nil {
		return nil, fmt.Errorf("cargo build: %w", err)
	}

	image := dag.Container().
		From("cgr.dev/chainguard/wolfi-base").
		WithExec([]string{"apk", "add", "--no-cache", "ca-certificates", "curl"}).
		WithWorkdir("/app").
		WithFile("/usr/local/bin/redcell", builder.File("/tmp/redcell")).
		WithDirectory("/app/migrations", src.Directory("migrations")).
		WithDirectory("/app/static", src.Directory("static")).
		WithDirectory("/app/templates", src.Directory("templates")).
		WithExec([]string{"mkdir", "-p", "/app/data"}).
		WithEnvVariable("REDTEAM__DATABASE__URL", "sqlite:///app/data/redcell.db").
		WithEnvVariable("REDTEAM__SERVER__HOST", "0.0.0.0").
		WithEnvVariable("REDTEAM__SERVER__PORT", "3000").
		WithExposedPort(3000).
		WithDefaultArgs([]string{"redcell"})

	return image, nil
}

// Publish builds and publishes the Redcell image to a container registry.
func (m *Redcell) Publish(
	ctx context.Context,
	src *dagger.Directory,
	registryToken *dagger.Secret,
	// +default="ghcr.io"
	// +optional
	imageRegistry string,
	// +default="redcell/redcell"
	// +optional
	imageRepository string,
	// +default="branded"
	// +optional
	imageTag string,
	// +default="x"
	// +optional
	registryUsername string,
) (string, error) {
	src = withoutGit(src)

	if imageRegistry == "" {
		imageRegistry = "ghcr.io"
	}
	if imageRepository == "" {
		imageRepository = "redcell/redcell"
	}
	if imageTag == "" {
		imageTag = "branded"
	}
	if registryUsername == "" {
		registryUsername = "x"
	}

	img, err := m.Build(ctx, src)
	if err != nil {
		return "", err
	}

	ref := fmt.Sprintf("%s/%s:%s", imageRegistry, imageRepository, imageTag)
	if registryToken != nil {
		img = img.WithRegistryAuth(imageRegistry, registryUsername, registryToken)
	}

	return img.Publish(ctx, ref)
}

// Deploy builds and deploys Redcell to Fly.io.
func (m *Redcell) DeployApp(
	ctx context.Context,
	src *dagger.Directory,
	flyToken *dagger.Secret,
	// Container registry token. If omitted, flyToken is used as a fallback.
	// +optional
	registryToken *dagger.Secret,
	// +default="redcell"
	// +optional
	flyAppName string,
	// +default="registry.fly.io/redcell"
	// +optional
	flyImageRef string,
) (string, error) {
	src = withoutGit(src)

	if flyAppName == "" {
		flyAppName = "redcell"
	}
	if flyImageRef == "" {
		flyImageRef = "registry.fly.io/redcell"
	}

	authToken := registryToken
	if authToken == nil {
		authToken = flyToken
	}

	img, err := m.Build(ctx, src)
	if err != nil {
		return "", fmt.Errorf("build redcell: %w", err)
	}

	tag := computeTag()
	flyRef := fmt.Sprintf("%s:%s", flyImageRef, tag)
	if _, err := img.
		WithRegistryAuth("registry.fly.io", "x", flyToken).
		Publish(ctx, flyRef); err != nil {
		return "", fmt.Errorf("publish redcell to fly registry: %w", err)
	}

	// Also publish to GHCR for record keeping.
	ghcrRef := fmt.Sprintf("ghcr.io/redcell/redcell:%s", tag)
	if _, err := img.
		WithRegistryAuth("ghcr.io", "x", authToken).
		Publish(ctx, ghcrRef); err != nil {
		fmt.Fprintf(os.Stderr, "warning: failed to publish redcell to ghcr: %v\n", err)
	}

	out, err := dag.Container().
		From(flyctlImage).
		WithMountedDirectory("/src", src).
		WithWorkdir("/src").
		WithSecretVariable("FLY_API_TOKEN", flyToken).
		WithEnvVariable("CACHE_BUSTER", fmt.Sprintf("%d", time.Now().Unix())).
		WithExec([]string{"/flyctl", "deploy", "--app", flyAppName, "--image", flyRef, "--yes"}).
		Stdout(ctx)
	if err != nil {
		return "", fmt.Errorf("fly deploy: %w", err)
	}
	return out, nil
}

// DeployPocketId builds and publishes the branded Pocket ID image, then rolls it
// out on Fly.io.
func (m *Redcell) DeployPocketId(
	ctx context.Context,
	src *dagger.Directory,
	flyToken *dagger.Secret,
	// Container registry token. If omitted, flyToken is used as a fallback.
	// +optional
	registryToken *dagger.Secret,
	// Pocket ID upstream version to build.
	// +default="2.9.0"
	// +optional
	version string,
	// Image tag to publish and deploy.
	// +default="branded"
	// +optional
	imageTag string,
	// Force a fresh Pocket ID image build.
	// +default=false
	// +optional
	forceRebuild bool,
) (string, error) {
	src = withoutGit(src)

	if version == "" {
		version = "2.9.0"
	}
	if imageTag == "" {
		imageTag = "branded"
	}

	authToken := registryToken
	if authToken == nil {
		authToken = flyToken
	}

	img := dag.PocketID().Build(dagger.PocketIDBuildOpts{
		Version:      version,
		Overlay:      src.Directory("pocket-id/dagger/overlay"),
		ForceRebuild: forceRebuild,
	})

	flyRef := fmt.Sprintf("registry.fly.io/redcell-pocket-id:%s", imageTag)
	if _, err := img.
		WithRegistryAuth("registry.fly.io", "x", flyToken).
		Publish(ctx, flyRef); err != nil {
		return "", fmt.Errorf("publish pocket-id to fly registry: %w", err)
	}

	ghcrRef := fmt.Sprintf("ghcr.io/redcell/pocket-id:%s", imageTag)
	if _, err := img.
		WithRegistryAuth("ghcr.io", "x", authToken).
		Publish(ctx, ghcrRef); err != nil {
		fmt.Fprintf(os.Stderr, "warning: failed to publish pocket-id to ghcr: %v\n", err)
	}

	out, err := dag.Container().
		From(flyctlImage).
		WithMountedDirectory("/src", src).
		WithWorkdir("/src/pocket-id").
		WithSecretVariable("FLY_API_TOKEN", flyToken).
		WithExec([]string{"/flyctl", "deploy", "--app", "redcell-pocket-id", "--image", flyRef, "--yes"}).
		Stdout(ctx)
	if err != nil {
		return "", fmt.Errorf("fly deploy pocket-id: %w", err)
	}
	return out, nil
}

// BootstrapPocketId ensures the deployed Pocket ID instance has an initial admin
// user and the public OIDC client required by Redcell.
func (m *Redcell) BootstrapPocketId(
	ctx context.Context,
	// Email address for the initial admin user.
	// +default="admin@redcells.net"
	// +optional
	adminEmail string,
	// Public Pocket ID base URL.
	// +default="https://pocketid.redcells.net"
	// +optional
	pocketIdBaseUrl string,
) (string, error) {
	if adminEmail == "" {
		adminEmail = "admin@redcells.net"
	}
	if pocketIdBaseUrl == "" {
		pocketIdBaseUrl = "https://pocketid.redcells.net"
	}

	script := fmt.Sprintf(`
set -e
apk add --no-cache curl jq

admin_email=%q
base_url=%q
max_wait=60

healthy=false
for i in $(seq 1 $max_wait); do
  if curl -fsS "%s/health" >/dev/null 2>&1; then
    healthy=true
    break
  fi
  sleep 2
done
if [ "$healthy" != "true" ]; then
  echo "pocket-id did not become healthy at %s/health"
  exit 1
fi

setup_status=$(curl -s -o /dev/null -w "%%{http_code}" "%s/api/signup/setup")
if [ "$setup_status" = "204" ]; then
  curl -fsS -X POST "%s/api/signup/setup" \
    -H "Content-Type: application/json" \
    -d '{"username":"admin","email":"%s","firstName":"Admin","lastName":"User"}' >/dev/null
fi

access_cookie=$(curl -s -c /tmp/jar.txt -b /tmp/jar.txt -o /dev/null -w "%%{http_code}" "%s/api/signup/setup")
has_token=""
if grep -q "__Host-access_token" /tmp/jar.txt 2>/dev/null; then
  has_token=true
fi

if [ "$has_token" != "true" ]; then
  meta_status=$(curl -s -o /dev/null -w "%%{http_code}" "%s/api/oidc/clients/redcell/meta")
  if [ "$meta_status" = "200" ]; then
    echo "Pocket ID already bootstrapped"
    exit 0
  fi
  echo "no admin access token available and OIDC client metadata returned $meta_status"
  exit 1
fi

client_status=$(curl -s -b /tmp/jar.txt -o /dev/null -w "%%{http_code}" "%s/api/oidc/clients/redcell")
client_payload='{"id":"redcell","name":"Redcell","callbackURLs":["https://redcells.net/auth/callback"],"isPublic":true,"pkceEnabled":true,"isGroupRestricted":false}'

if [ "$client_status" = "404" ]; then
  curl -fsS -b /tmp/jar.txt -X POST "%s/api/oidc/clients" \
    -H "Content-Type: application/json" \
    -d "$client_payload" >/dev/null
  echo "created initial admin and OIDC client"
else
  curl -fsS -b /tmp/jar.txt -X PUT "%s/api/oidc/clients/redcell" \
    -H "Content-Type: application/json" \
    -d "$client_payload" >/dev/null
  echo "created initial admin and updated OIDC client"
fi
`, adminEmail, pocketIdBaseUrl, pocketIdBaseUrl, pocketIdBaseUrl, pocketIdBaseUrl, pocketIdBaseUrl, adminEmail, pocketIdBaseUrl, pocketIdBaseUrl, pocketIdBaseUrl, pocketIdBaseUrl, pocketIdBaseUrl)

	out, err := dag.Container().
		From("alpine:3.19").
		WithExec([]string{"sh", "-c", script}).
		Stdout(ctx)
	if err != nil {
		return "", fmt.Errorf("bootstrap pocket-id: %w", err)
	}
	return out, nil
}

// VerifyDeployment performs post-deploy HTTP 200 checks against the live endpoints.
func (m *Redcell) VerifyDeployment(ctx context.Context) (string, error) {
	checker := dag.Container().
		From("alpine:3.19").
		WithExec([]string{"apk", "add", "--no-cache", "curl"}).
		WithExec([]string{"sh", "-c", `
set -e
fail() {
  echo "VERIFY FAIL: $1"
  exit 1
}

for path in / /login /tos /docs; do
  echo "Checking redcell $path..."
  STATUS=$(curl -s -o /dev/null --max-redirs 0 -w "%{http_code}" "https://redcells.net$path")
  if [ "$STATUS" != "200" ]; then
    fail "redcell $path returned $STATUS (expected 200)"
  fi
  echo "redcell $path OK"
done

# /register redirects anonymous visitors to /login because sign-up is handled via OIDC.
echo "Checking redcell /register redirect..."
STATUS=$(curl -s -o /dev/null --max-redirs 0 -w "%{http_code}" "https://redcells.net/register")
if [ "$STATUS" != "302" ] && [ "$STATUS" != "303" ]; then
  fail "redcell /register returned $STATUS (expected 302/303 redirect)"
fi
echo "redcell /register redirect OK"

echo "Checking redcell health..."
STATUS=$(curl -s -o /dev/null -w "%{http_code}" https://redcells.net/health)
if [ "$STATUS" != "200" ]; then
  fail "redcell health returned $STATUS"
fi
echo "redcell health OK"

echo "Checking redcell stylesheet has new utility classes..."
CSS=$(curl -s "https://redcells.net/static/output.css?v=3")
for cls in aurora-blob animate-aurora light-ray glass-card text-gradient animate-orbit animate-pulse-core animate-ray mesh-bg animate-float shadow-accent-glow; do
  if ! echo "$CSS" | grep -q "$cls"; then
    fail "missing CSS class $cls in /static/output.css?v=2"
  fi
  echo "  $cls OK"
done

echo "Checking pocket-id authorize page..."
STATUS=$(curl -s -o /dev/null -w "%{http_code}" 'https://pocketid.redcells.net/authorize?client_id=redcell&response_type=code&scope=openid+email+profile&redirect_uri=https%3A%2F%2Fredcells.net%2Fauth%2Fcallback')
if [ "$STATUS" != "200" ]; then
  fail "pocket-id authorize page returned $STATUS (expected 200)"
fi
echo "pocket-id authorize page OK"

echo ""
echo "All deployment verification checks passed."
`})

	out, err := checker.Stdout(ctx)
	if err != nil {
		return "", fmt.Errorf("deployment verification failed: %w", err)
	}
	return out, nil
}

// Deploy orchestrates Redcell and Pocket ID deploys, then bootstraps Pocket ID.
func (m *Redcell) Deploy(
	ctx context.Context,
	src *dagger.Directory,
	flyToken *dagger.Secret,
	// GHCR token used to publish images. If omitted, flyToken is used as fallback.
	// +optional
	ghcrToken *dagger.Secret,
	// Pocket ID upstream version to build.
	// +default="2.9.0"
	// +optional
	pocketIdVersion string,
) (string, error) {
	src = withoutGit(src)

	deployCtx, cancelDeploy := context.WithCancel(ctx)
	defer cancelDeploy()
	g, _ := errgroup(deployCtx)

	var appOut, pocketOut string
	var appErr, pocketErr error

	g.Go(func() error {
		appOut, appErr = m.DeployApp(deployCtx, src, flyToken, ghcrToken, "", "")
		return appErr
	})
	g.Go(func() error {
		pocketOut, pocketErr = m.DeployPocketId(deployCtx, src, flyToken, ghcrToken, pocketIdVersion, "", false)
		return pocketErr
	})

	if err := g.Wait(); err != nil {
		if appErr != nil {
			return "", fmt.Errorf("redcell deploy: %w", appErr)
		}
		return "", fmt.Errorf("pocket-id deploy: %w", pocketErr)
	}

	bootstrapOut, err := m.BootstrapPocketId(ctx, "", "")
	if err != nil {
		return "", fmt.Errorf("pocket-id bootstrap: %w", err)
	}

	verifyOut, err := m.VerifyDeployment(ctx)
	if err != nil {
		return "", fmt.Errorf("deploy succeeded but verification failed: %w", err)
	}

	return fmt.Sprintf("=== Redcell ===\n%s\n\n=== Pocket ID ===\n%s\n\n=== Bootstrap ===\n%s\n\n=== Verification ===\n%s", appOut, pocketOut, bootstrapOut, verifyOut), nil
}

func computeTag() string {
	refType := os.Getenv("GITHUB_REF_TYPE")
	refName := os.Getenv("GITHUB_REF_NAME")
	if refType == "tag" {
		tag := strings.TrimPrefix(refName, "v")
		if tag != "" {
			return tag
		}
	}

	sha := os.Getenv("GITHUB_SHA")
	if len(sha) >= 7 {
		return fmt.Sprintf("0.1.0-%s", sha[:7])
	}

	return fmt.Sprintf("0.1.0-dev-%d", time.Now().Unix())
}

// errgroup returns a minimal error group for concurrent Dagger calls.
func errgroup(ctx context.Context) (*waitGroup, context.Context) {
	ctx, cancel := context.WithCancel(ctx)
	return &waitGroup{ctx: ctx, cancel: cancel}, ctx
}

type waitGroup struct {
	ctx    context.Context
	cancel func()
	wg     sync.WaitGroup
	err    error
	mu     sync.Mutex
}

func (w *waitGroup) Go(fn func() error) {
	w.wg.Add(1)
	go func() {
		defer w.wg.Done()
		if err := fn(); err != nil {
			w.mu.Lock()
			if w.err == nil {
				w.err = err
				w.cancel()
			}
			w.mu.Unlock()
		}
	}()
}

func (w *waitGroup) Wait() error {
	w.wg.Wait()
	w.cancel()
	return w.err
}
