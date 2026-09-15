# Majordomus ChatGPT Project Pack

This archive is intended to be uploaded as reference material into a ChatGPT Project.

## Setup

1. Open the Majordomus project.
2. Open Project settings.
3. Paste the contents of `PROJECT_INSTRUCTIONS.txt` into Project Instructions.
4. Upload the five numbered Markdown files as Project files:
   - `01_ARCHITECTURE_PRINCIPLES.md`
   - `02_PRISMATIC_PORTING_POLICY.md`
   - `03_DEVELOPMENT_AND_REVIEW.md`
   - `04_TESTING_AND_GUARANTEES.md`
   - `05_ROADMAP.md`
5. Keep this README locally or upload it too if useful.

## Why the split exists

The short Project Instructions contain the non-negotiable operating rules.

Detailed material is split into reference files so the instructions remain comfortably below the current custom-instruction character ceiling while retaining richer architectural context.

The most important boundary is simple:

**Prismatic may inspire Majordomus, but Majordomus must never depend on Prismatic.**

Any useful Prismatic functionality must be adapted, ported, or independently reimplemented inside Majordomus with its own interfaces, tests, documentation, and lifecycle.
