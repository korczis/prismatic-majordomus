+++
title = "fly-deployment — The container image is a projection of the repository model, and its smoke is its evidence"
description = "One canonical deployment object under .ai/repo/deployments/ describes the image; a typed Rust model parses and validates it; the Dockerfile, its ignore file, the GHCR image that docker-image.yml and release.yml build, its smoke derived from the route registry, the doctor checks, the documentation and the cockpit view are projections of that object and of the capability registry, each checked for drift. The published image runs as a non-root user, carries no secret and no developer path, answers live and ready in its smoke, and has measured size, cold-start and memory baselines. Re-scoped 2026-10-14 from a Fly.io deployment, which the owner disconnected on 2026-10-12."
weight = 13
template = "milestone.html"
[extra]
plan_id = "fly-deployment"
source = ".ai/repo/project/milestones/fly-deployment.yaml"
+++
