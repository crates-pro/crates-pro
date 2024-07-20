rust_binary(
    name = "crates-pro",
    srcs = [
        "src/cli.rs",
        "src/core_controller.rs",
        "src/main.rs",
    ],
    crate_root = "src/main.rs",
    edition = "2021",
    deps = [
        "//submodules/crates-pro:crates_sync",
        "//submodules/crates-pro:model",
        "//submodules/crates-pro:repo_import",
        "//submodules/crates-pro:tudriver",
        "//third-party:dotenvy",
        "//third-party:neo4rs",
        "//third-party:rdkafka",
        "//third-party:serde_json",
        "//third-party:structopt",
        "//third-party:tokio",
        "//third-party:tracing",
        "//third-party:tracing-subscriber",
    ],
    visibility = ["PUBLIC"],
)

alias(
    name = "crates_sync",
    actual = "//submodules/crates-pro/crates_sync:crates_sync",
    visibility = ["PUBLIC"],
)

alias(
    name = "model",
    actual = "//submodules/crates-pro/model:model",
    visibility = ["PUBLIC"],
)

alias(
    name = "repo_import",
    actual = "//submodules/crates-pro/repo_import:repo_import",
    visibility = ["PUBLIC"],
)

alias(
    name = "tudriver",
    actual = "//submodules/crates-pro/tudriver:tudriver",
    visibility = ["PUBLIC"],
)
