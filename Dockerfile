# ---- frontend build ----
FROM node:22-slim AS frontend
WORKDIR /app
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

# ---- backend ----
FROM julia:1.11-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends curl git \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app/backend

# Generic CPU target so an image built in CI runs on any host without
# recompiling (precompile cache misses otherwise).
ENV JULIA_CPU_TARGET="generic;haswell,clone_all"

# Dependency layer, keyed on Project.toml only. PowerDiff and PowerIO are not
# registered yet, so they come from git; the local dev Manifest points at
# local paths and is intentionally not copied.
ARG POWERIO_JL_URL=https://github.com/eigenergy/PowerIO.jl.git
ARG POWERDIFF_URL=https://github.com/grid-opt-alg-lab/PowerDiff.jl.git
COPY backend/Project.toml ./
RUN julia -e "using Pkg; Pkg.activate(\".\"); \
    Pkg.add(url=\"$POWERIO_JL_URL\"); \
    Pkg.add(url=\"$POWERDIFF_URL\"); \
    Pkg.instantiate(); Pkg.precompile()"

# Pull the pglib artifact at build time so first boot does not hit the network.
RUN julia --project=. -e "using PowerDiff; PowerDiff.get_path(:pglib)"

COPY backend/src ./src
COPY backend/bootstrap.jl ./
COPY --from=frontend /app/build /app/frontend/build

EXPOSE 8000
# Long start period: Julia loads JuMP and Ipopt, then solves the bundled case.
HEALTHCHECK --interval=30s --timeout=5s --start-period=240s --retries=5 \
    CMD curl -fsS http://localhost:8000/api/health | grep -q '"ok"' || exit 1

CMD ["julia", "--project=.", "bootstrap.jl"]
