package main

import (
	"context"
	"fmt"
	"time"

	"pocket-id/internal/dagger"
)

type PocketId struct{}

// Build the branded redcell/pocket-id image from the upstream Pocket ID
// source and the local overlay. Returns a container that can be exported or
// published via the Dagger CLI.
func (m *PocketId) Build(
	ctx context.Context,
	// Pocket ID upstream version tag to build from.
	// +default="2.9.0"
	// +optional
	version string,
	// Local overlay directory to apply on top of upstream Pocket ID. If omitted,
	// the overlay bundled with this module is used.
	// +optional
	overlay *dagger.Directory,
	// Force a fresh build even when upstream source and overlay are unchanged.
	// Use sparingly; by default layers are reused when inputs are identical.
	// +default=false
	// +optional
	forceRebuild bool,
) (*dagger.Container, error) {
	if version == "" {
		version = "2.9.0"
	}

	if overlay == nil {
		overlay = dag.CurrentModule().Source().Directory("overlay")
	}

	upstream := dag.Git("https://github.com/pocket-id/pocket-id.git").
		Tag("v" + version).
		Tree()

	src := upstream.WithDirectory(".", overlay)

	opts := dagger.DirectoryDockerBuildOpts{
		Dockerfile: "docker/Dockerfile",
		Platform:   "linux/amd64",
	}
	if forceRebuild {
		// Only bust the Docker cache when explicitly requested. The upstream
		// tag and overlay already provide deterministic cache keys.
		opts.BuildArgs = []dagger.BuildArg{{Name: "CACHE_BUST", Value: fmt.Sprintf("%d", time.Now().Unix())}}
	}

	return src.DockerBuild(opts), nil
}

// Publish the branded redcell/pocket-id image to a container registry.
// Defaults to ghcr.io/redcell/pocket-id:branded.
func (m *PocketId) Publish(
	ctx context.Context,
	// Registry address including repository path, e.g. ghcr.io/redcell/pocket-id.
	// +default="ghcr.io/redcell/pocket-id"
	// +optional
	registry string,
	// Image tag.
	// +default="branded"
	// +optional
	tag string,
	// Registry username. If empty, no authentication is attempted.
	// +optional
	username string,
	// Registry password/secret. If empty, no authentication is attempted.
	// +optional
	secret *dagger.Secret,
	// Local overlay directory to apply on top of upstream Pocket ID. If omitted,
	// the overlay bundled with this module is used.
	// +optional
	overlay *dagger.Directory,
	// Force a fresh build even when upstream source and overlay are unchanged.
	// +default=false
	// +optional
	forceRebuild bool,
) (string, error) {
	if registry == "" {
		registry = "ghcr.io/redcell/pocket-id"
	}
	if tag == "" {
		tag = "branded"
	}

	img, err := m.Build(ctx, "", overlay, forceRebuild)
	if err != nil {
		return "", err
	}

	ref := fmt.Sprintf("%s:%s", registry, tag)
	if username != "" && secret != nil {
		img = img.WithRegistryAuth(registry, username, secret)
	}

	return img.Publish(ctx, ref)
}
