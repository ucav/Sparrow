#[test]
fn one_click_scripts_target_the_real_release_artifacts() {
    let sh = std::fs::read_to_string("install.sh").expect("install.sh must exist");
    let ps1 = std::fs::read_to_string("install.ps1").expect("install.ps1 must exist");

    for script in [&sh, &ps1] {
        assert!(
            script.contains("ucav/Sparrow"),
            "one-click scripts must target the public ucav/Sparrow repository"
        );
        assert!(
            !script.contains("sparrow-dev/sparrow"),
            "one-click scripts must not target the old placeholder repository"
        );
        assert!(
            script.contains("sparrow launch"),
            "one-click scripts must guide users to the simplified launch command"
        );
    }

    for artifact in [
        "sparrow-linux-x86_64",
        "sparrow-macos-arm64",
        "sparrow-windows-x86_64.exe",
    ] {
        assert!(
            sh.contains(artifact) || ps1.contains(artifact),
            "one-click scripts must reference release artifact `{artifact}`"
        );
    }
}

#[test]
fn docs_expose_one_click_install_and_launch() {
    let readme = std::fs::read_to_string("README.md").expect("README.md must exist");
    let docs = std::fs::read_to_string("docs/index.html").expect("docs/index.html must exist");
    let cli_ref =
        std::fs::read_to_string("docs/cli-reference.md").expect("cli reference must exist");

    for text in [&readme, &docs] {
        assert!(text.contains("install.ps1"));
        assert!(text.contains("install.sh"));
        assert!(text.contains("sparrow launch"));
    }

    assert!(
        cli_ref.contains("`sparrow launch`"),
        "CLI reference must document the simplified launch command"
    );
}
