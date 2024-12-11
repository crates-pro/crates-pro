load("@prelude//rust:cargo_package.bzl", "cargo")

# package definitions
filegroup(
    name = "crates_pro-0.1.0.crate",
    srcs = [
        "src/cli.rs",
        "src/core_controller.rs",
        "src/main.rs",
    ],
)

pkg_deps = [
    "//project/crates-pro:analysis",
    "//project/crates-pro:data_transporter",
    "//project/crates-pro:model",
    "//project/crates-pro:repo_import",
    "//project/crates-pro:search",
    "//project/crates-pro:tudriver",
    "//third-party:dotenvy",
    "//third-party:neo4rs",
    "//third-party:rdkafka",
    "//third-party:serde_json",
    "//third-party:structopt",
    "//third-party:tokio",
    "//third-party:tracing",
    "//third-party:tracing-subscriber",
    "//third-party:serial_test"
]

# targets
cargo.rust_binary(
    name = "crates_pro",
    srcs = [":crates_pro-0.1.0.crate"],
    crate_root = "crates_pro-0.1.0.crate/src/main.rs",
    edition = "2021",
    deps = pkg_deps,
    visibility = ["PUBLIC"],
)

# aliases
alias(
    name = "analysis",
    actual = "//project/crates-pro/analysis:analysis",
    visibility = ["PUBLIC"],
)

alias(
    name = "data_transporter",
    actual = "//project/crates-pro/data_transporter:data_transporter",
    visibility = ["PUBLIC"],
)

alias(
    name = "model",
    actual = "//project/crates-pro/model:model",
    visibility = ["PUBLIC"],
)

alias(
    name = "repo_import",
    actual = "//project/crates-pro/repo_import:repo_import",
    visibility = ["PUBLIC"],
)


alias(
    name = "tudriver",
    actual = "//project/crates-pro/tudriver:tudriver",
    visibility = ["PUBLIC"],
)
alias(
    name = "search",
    actual = "//project/crates-pro/search:search",
    visibility = ["PUBLIC"],
)
