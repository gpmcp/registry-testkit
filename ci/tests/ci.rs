use gh_workflow::*;

#[test]
fn main() {
    // Create a workflow focused on testing and coverage
    let test_job = Job::new("test")
        .add_step(Step::checkout())
        .add_step(
            Step::new("Setup Rust Toolchain")
                .uses("actions-rust-lang", "setup-rust-toolchain", "v1")
                .with(
                    Input::default()
                        .add("toolchain", "nightly")
                        .add("components", "llvm-tools-preview")
                        .add("cache", "true"),
                ),
        )
        .add_step(
            Step::new("Install protoc")
                .run("sudo apt-get update && sudo apt-get install -y protobuf-compiler"),
        )
        .add_step(
            Step::new("Cache cargo-llvm-cov")
                .uses("actions", "cache", "v4")
                .with(
                    Input::default()
                        .add("path", "~/.cargo/bin/cargo-llvm-cov")
                        .add(
                            "key",
                            "${{ runner.os }}-cargo-llvm-cov-${{ hashFiles('**/Cargo.lock') }}",
                        )
                        .add("restore-keys", "${{ runner.os }}-cargo-llvm-cov-"),
                ),
        )
        .add_step(Step::new("Install cargo-llvm-cov").run("cargo install cargo-llvm-cov || true"))
        .add_step(Step::new("Generate coverage").run(
            "cargo +nightly llvm-cov --all-features --workspace --lcov --output-path lcov.info",
        ))
        .add_step(
            Step::new("Upload Coverage to Codecov")
                .uses("Wandalen", "wretry.action", "v3")
                .with(
                    Input::default()
                        .add("action", "codecov/codecov-action@v4")
                        .add("attempt_limit", "3")
                        .add("attempt_delay", "10000")
                        .add(
                            "with",
                            "token: ${{ secrets.CODECOV_TOKEN }}\nfiles: lcov.info",
                        ),
                ),
        );

    let workflow = Workflow::new("ci")
        .on(Event::default()
            .push(Push::default().add_branch("main"))
            .pull_request(PullRequest::default().add_branch("main")))
        .add_job("test", test_job);

    workflow.generate().unwrap();
}
