group "default" {
  targets = ["axon"]
}

target "axon" {
  dockerfile = "dev-support/containers/alpine/Containerfile"
  platforms  = ["linux/amd64"]
  target     = "axon"
  contexts = {
    rust   = "docker-image://docker.io/library/rust:1.89.0-alpine3.22"
    alpine = "docker-image://docker.io/library/alpine:3.22"
  }
  args = {
    RUSTC_WRAPPER         = "/usr/bin/sccache"
    SCCACHE_GHA_ENABLED   = "off"
    ACTIONS_CACHE_URL     = null
    ACTIONS_RUNTIME_TOKEN = null
  }
  labels = {
    "description"                     = "Container image for Axon"
    "image.type"                      = "final"
    "image.authors"                   = "46590321+xrelkd@users.noreply.github.com"
    "image.vendor"                    = "xrelkd"
    "image.description"               = "Command-line tool designed to simplify your interactions with Kubernetes"
    "org.opencontainers.image.source" = "https://github.com/xrelkd/axon"
  }
}
