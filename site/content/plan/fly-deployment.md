+++
title = "fly-deployment — A deployment is a projection of the repository model, and a reachable URL is its evidence"
description = "One canonical deployment object under .ai/repo/deployments/ describes the deployment; a typed Rust model parses and validates it; the container image, the Fly configuration, the smoke suite, the doctor checks, the documentation, the cockpit view and the CI jobs are all projections of that object and of the capability registry, and every one of them is checked for drift. The service runs on Fly.io on a single shared-CPU Machine that stops when idle, its health and readiness are verified from outside, the smoke suite is derived from the route registry rather than written twice, and the deployed URL answers."
weight = 13
template = "milestone.html"
[extra]
plan_id = "fly-deployment"
source = ".ai/repo/project/milestones/fly-deployment.yaml"
+++
