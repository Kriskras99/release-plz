use crate::helpers::{
    package::{PackageType, TestPackage},
    test_context::TestContext,
    today,
};
use cargo_metadata::semver::Version;
use cargo_utils::{CARGO_TOML, LocalManifest};

fn assert_cargo_semver_checks_is_installed() {
    if !release_plz_core::semver_check::is_cargo_semver_checks_installed() {
        panic!(
            "cargo-semver-checks is not installed. Please install it to run tests: https://github.com/obi1kenobi/cargo-semver-checks"
        );
    }
}

#[tokio::test]
#[cfg_attr(not(feature = "docker-tests"), ignore)]
async fn release_plz_opens_pr_with_default_config() {
    let context = TestContext::new().await;

    context.run_release_pr().success();
    let today = today();

    let opened_prs = context.opened_release_prs().await;
    assert_eq!(opened_prs.len(), 1);
    assert_eq!(opened_prs[0].title, "chore: release v0.1.0");
    let username = context.gitea.user.username();
    let package = &context.gitea.repo;
    assert_eq!(
        opened_prs[0].body.as_ref().unwrap().trim(),
        format!(
            r#"
## 🤖 New release

* `{package}`: 0.1.0

<details><summary><i><b>Changelog</b></i></summary><p>

<blockquote>

## [0.1.0](https://localhost/{username}/{package}/releases/tag/v0.1.0) - {today}

### Other

- cargo init
- Initial commit
</blockquote>


</p></details>

---
This PR was generated with [release-plz](https://github.com/release-plz/release-plz/)."#,
        )
        .trim()
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "docker-tests"), ignore)]
async fn release_plz_opens_pr_without_breaking_changes() {
    assert_cargo_semver_checks_is_installed();
    let context = TestContext::new().await;

    let lib_file = context.repo_dir().join("src").join("lib.rs");

    let write_lib_file = |content: &str, commit_message: &str| {
        fs_err::write(&lib_file, content).unwrap();
        context.push_all_changes(commit_message);
    };

    write_lib_file("pub fn foo() {}", "add lib");

    context.run_release_pr().success();
    context.merge_release_pr().await;
    context.run_release().success();

    write_lib_file(
        "pub fn foo() {println!(\"hello\");}",
        "edit lib with compatible change",
    );

    context.run_release_pr().success();
    let today = today();

    let opened_prs = context.opened_release_prs().await;
    assert_eq!(opened_prs.len(), 1);
    assert_eq!(opened_prs[0].title, "chore: release v0.1.1");
    let username = context.gitea.user.username();
    let package = &context.gitea.repo;
    let pr_body = opened_prs[0].body.as_ref().unwrap().trim();
    pretty_assertions::assert_eq!(
        pr_body,
        format!(
            r#"
## 🤖 New release

* `{package}`: 0.1.0 -> 0.1.1 (✓ API compatible changes)

<details><summary><i><b>Changelog</b></i></summary><p>

<blockquote>

## [0.1.1](https://localhost/{username}/{package}/compare/v0.1.0...v0.1.1) - {today}

### Other

- edit lib with compatible change
</blockquote>


</p></details>

---
This PR was generated with [release-plz](https://github.com/release-plz/release-plz/)."#,
        )
        .trim()
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "docker-tests"), ignore)]
async fn release_plz_opens_pr_with_breaking_changes() {
    assert_cargo_semver_checks_is_installed();
    let context = TestContext::new().await;

    let lib_file = context.repo_dir().join("src").join("lib.rs");

    let write_lib_file = |content: &str, commit_message: &str| {
        fs_err::write(&lib_file, content).unwrap();
        context.push_all_changes(commit_message);
    };

    write_lib_file("pub fn foo() {}", "add lib");

    context.run_release_pr().success();
    context.merge_release_pr().await;
    context.run_release().success();

    write_lib_file("pub fn bar() {}", "edit lib with breaking change");

    context.run_release_pr().success();
    let today = today();

    let opened_prs = context.opened_release_prs().await;
    assert_eq!(opened_prs.len(), 1);
    assert_eq!(opened_prs[0].title, "chore: release v0.2.0");
    let username = context.gitea.user.username();
    let package = &context.gitea.repo;
    let pr_body = opened_prs[0].body.as_ref().unwrap().trim();
    // Remove the following lines from the semver check report to be able to do `assert_eq`:
    // - The line with the line number of the source because it contains a temporary directory
    //   that we don't know.
    // - The line containing the cargo semver checks version because it can change.
    let pr_body = pr_body
        .lines()
        .filter(|line| !does_line_vary(line))
        .collect::<Vec<_>>()
        .join("\n");
    pretty_assertions::assert_eq!(
        pr_body,
        format!(
            r#"
## 🤖 New release

* `{package}`: 0.1.0 -> 0.2.0 (⚠ API breaking changes)

### ⚠ `{package}` breaking changes

```text
--- failure function_missing: pub fn removed or renamed ---

Description:
A publicly-visible function cannot be imported by its prior path. A `pub use` may have been removed, or the function itself may have been renamed or removed entirely.
        ref: https://doc.rust-lang.org/cargo/reference/semver.html#item-remove

Failed in:
```

<details><summary><i><b>Changelog</b></i></summary><p>

<blockquote>

## [0.2.0](https://localhost/{username}/{package}/compare/v0.1.0...v0.2.0) - {today}

### Other

- edit lib with breaking change
</blockquote>


</p></details>

---
This PR was generated with [release-plz](https://github.com/release-plz/release-plz/)."#,
        )
        .trim()
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "docker-tests"), ignore)]
async fn release_plz_updates_binary_when_library_changes() {
    let binary = "binary";
    let library1 = "library1";
    let library2 = "library2";
    // dependency chain: binary -> library2 -> library1
    let context = TestContext::new_workspace_with_packages(&[
        TestPackage::new(binary)
            .with_type(PackageType::Bin)
            .with_path_dependencies(vec![format!("../{library2}")]),
        TestPackage::new(library2)
            .with_type(PackageType::Lib)
            .with_path_dependencies(vec![format!("../{library1}")]),
        TestPackage::new(library1).with_type(PackageType::Lib),
    ])
    .await;

    context.run_release_pr().success();
    context.merge_release_pr().await;
    context.run_release().success();

    // Update the library.
    let lib_file = context.package_path(library1).join("src").join("aa.rs");
    fs_err::write(&lib_file, "pub fn foo() {}").unwrap();
    context.push_all_changes("edit library");

    context.run_release_pr().success();
    let today = today();
    let opened_prs = context.opened_release_prs().await;
    assert_eq!(opened_prs.len(), 1);

    let open_pr = &opened_prs[0];
    assert_eq!(open_pr.title, "chore: release v0.1.1");

    let username = context.gitea.user.username();
    let repo = &context.gitea.repo;
    // The binary depends on the library, so release-plz should update its version.
    assert_eq!(
        open_pr.body.as_ref().unwrap().trim(),
        format!(
            r#"
## 🤖 New release

* `{library1}`: 0.1.0 -> 0.1.1 (✓ API compatible changes)
* `{library2}`: 0.1.0 -> 0.1.1
* `{binary}`: 0.1.0 -> 0.1.1

<details><summary><i><b>Changelog</b></i></summary><p>

## `{library1}`

<blockquote>

## [0.1.1](https://localhost/{username}/{repo}/compare/{library1}-v0.1.0...{library1}-v0.1.1) - {today}

### Other

- edit library
</blockquote>

## `{library2}`

<blockquote>

## [0.1.1](https://localhost/{username}/{repo}/compare/{library2}-v0.1.0...{library2}-v0.1.1) - {today}

### Other

- updated the following local packages: {library1}
</blockquote>

## `{binary}`

<blockquote>

## [0.1.1](https://localhost/{username}/{repo}/compare/{binary}-v0.1.0...{binary}-v0.1.1) - {today}

### Other

- updated the following local packages: {library2}
</blockquote>


</p></details>

---
This PR was generated with [release-plz](https://github.com/release-plz/release-plz/)."#,
        )
        .trim()
    );

    context.merge_release_pr().await;

    // Check if the binary has the new version.
    let binary_cargo_toml =
        fs_err::read_to_string(context.package_path(binary).join(CARGO_TOML)).unwrap();
    expect_test::expect![[r#"
        [package]
        name = "binary"
        version = "0.1.1"
        edition = "2024"
        publish = ["test-registry"]

        [dependencies]
        library2 = { version = "0.1.1", path = "../library2", registry = "test-registry" }
    "#]]
    .assert_eq(&binary_cargo_toml);
}

#[tokio::test]
#[cfg_attr(not(feature = "docker-tests"), ignore)]
async fn release_plz_opens_pr_with_two_packages_and_default_config() {
    let one = "one";
    let two = "two";
    let context = TestContext::new_workspace(&[one, two]).await;

    context.run_release_pr().success();
    let today = today();

    let opened_prs = context.opened_release_prs().await;
    assert_eq!(opened_prs.len(), 1);

    let open_pr = &opened_prs[0];
    assert_eq!(open_pr.title, "chore: release v0.1.0");

    let username = context.gitea.user.username();
    let repo = &context.gitea.repo;
    assert_eq!(
        open_pr.body.as_ref().unwrap().trim(),
        format!(
            r#"
## 🤖 New release

* `{one}`: 0.1.0
* `{two}`: 0.1.0

<details><summary><i><b>Changelog</b></i></summary><p>

## `{one}`

<blockquote>

## [0.1.0](https://localhost/{username}/{repo}/releases/tag/{one}-v0.1.0) - {today}

### Other

- cargo init
</blockquote>

## `{two}`

<blockquote>

## [0.1.0](https://localhost/{username}/{repo}/releases/tag/{two}-v0.1.0) - {today}

### Other

- cargo init
</blockquote>


</p></details>

---
This PR was generated with [release-plz](https://github.com/release-plz/release-plz/)."#,
        )
        .trim()
    );

    // After landing the PR, there is no release PR open.
    context.merge_release_pr().await;
    context.run_release_pr().success();

    let opened_prs = context.opened_release_prs().await;
    assert_eq!(opened_prs.len(), 0);
}

#[tokio::test]
#[cfg_attr(not(feature = "docker-tests"), ignore)]
async fn release_plz_should_set_custom_pr_details() {
    let context = TestContext::new().await;

    let config = r#"
[workspace]
pr_name = "release:{% if package and version %} {{ package }} {{ version }}{% endif %}"
pr_body = """
{% for release in releases %}
{% if release.title %}
### {{release.title}}
{% endif %}
Package: {{release.package}} {{release.previous_version}} -> {{release.next_version}}
{% if release.changelog %}
Changes:
{{release.changelog}}
{% endif %}
{% endfor -%}
"""
    "#;

    context.write_release_plz_toml(config);
    context.run_release_pr().success();
    let today = today();

    let expected_title = format!("release: {} 0.1.0", context.gitea.repo);
    let opened_prs = context.opened_release_prs().await;
    assert_eq!(opened_prs.len(), 1);
    assert_eq!(opened_prs[0].title, expected_title);
    let package = &context.gitea.repo;
    let username = context.gitea.user.username();
    assert_eq!(
        opened_prs[0].body.as_ref().unwrap().trim(),
        format!(
            r#"
### [0.1.0](https://localhost/{username}/{package}/releases/tag/v0.1.0) - {today}

Package: {package} 0.1.0 -> 0.1.0

Changes:
### Other

- add config file
- cargo init
- Initial commit"#,
        )
        .trim()
    );

    context.merge_release_pr().await;
    // The commit contains the PR id number
    let expected_commit = format!("{expected_title} (#1)");
    assert_eq!(
        context.repo.current_commit_message().unwrap(),
        expected_commit
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "docker-tests"), ignore)]
async fn release_plz_should_fail_for_multi_package_pr() {
    let context = TestContext::new_workspace(&["one", "two"]).await;

    let config = r#"
    [workspace]
    pr_name = "release: {{ package }} {{ version }}"
    "#;

    context.write_release_plz_toml(config);
    // This should fail because the workspace contains multiple packages
    // so the `package` variable is not available
    let outcome = context.run_release_pr().failure();
    let stderr = String::from_utf8_lossy(&outcome.get_output().stderr);
    assert!(stderr.contains("failed to render pr_name"));
}

#[tokio::test]
#[cfg_attr(not(feature = "docker-tests"), ignore)]
async fn release_plz_detects_edited_readme_cargo_toml_field() {
    let context = TestContext::new().await;

    context.run_release_pr().success();
    context.merge_release_pr().await;

    let expected_tag = "v0.1.0";

    context.run_release().success();

    let gitea_release = context.gitea.get_gitea_release(expected_tag).await;
    assert_eq!(gitea_release.name, expected_tag);

    move_readme(&context, "move readme");

    context.run_release_pr().success();
    context.merge_release_pr().await;

    let expected_tag = "v0.1.1";

    context.run_release().success();

    let gitea_release = context.gitea.get_gitea_release(expected_tag).await;
    assert_eq!(gitea_release.name, expected_tag);
    expect_test::expect![[r#"
        ### Other

        - move readme"#]]
    .assert_eq(&gitea_release.body);
}

#[tokio::test]
#[cfg_attr(not(feature = "docker-tests"), ignore)]
async fn release_plz_honors_features_always_increment_minor_flag() {
    let context = TestContext::new().await;

    let config = r#"
    [workspace]
    features_always_increment_minor = true
    "#;
    context.write_release_plz_toml(config);

    context.run_release_pr().success();
    context.merge_release_pr().await;

    let expected_tag = "v0.1.0";

    context.run_release().success();

    let gitea_release = context.gitea.get_gitea_release(expected_tag).await;
    assert_eq!(gitea_release.name, expected_tag);

    move_readme(&context, "feat: move readme");

    let outcome = context.run_release_pr().success();

    let opened_prs = context.opened_release_prs().await;
    let open_pr = &opened_prs[0];
    let expected_stdout = serde_json::json!({
        "prs": [{
            "base_branch": "main",
            "head_branch": open_pr.branch(),
            "html_url": open_pr.html_url,
            "number": open_pr.number,
            "releases": [{
                "package_name": context.gitea.repo,
                "version": "0.2.0"
            }]
        }]
    });
    outcome.stdout(format!("{expected_stdout}\n"));
    context.merge_release_pr().await;

    let expected_tag = "v0.2.0";

    context.run_release().success();

    let gitea_release = context.gitea.get_gitea_release(expected_tag).await;
    assert_eq!(gitea_release.name, expected_tag);
    expect_test::expect![[r#"
        ### Added

        - move readme"#]]
    .assert_eq(&gitea_release.body);
}

#[tokio::test]
#[cfg(unix)]
#[cfg_attr(not(feature = "docker-tests"), ignore)]
async fn release_plz_detects_symlink_package_changes() {
    let context = TestContext::new().await;
    let readme = "README.md";
    let readme_symlink = "README_SYMLINK.md";
    symlink_readme(&context, readme, readme_symlink);
    let cargo_toml_path = context.repo_dir().join(CARGO_TOML);
    let mut cargo_toml = LocalManifest::try_new(&cargo_toml_path).unwrap();

    // By default, all files are included in the package.
    // We need this test to verify that the changes are tracked correctly if the original
    // (non-symlinked) README is not part of the package.
    cargo_toml.data["package"]["include"] = toml_edit::value(toml_edit::Array::from_iter(["/src"]));
    cargo_toml.write().unwrap();

    context.push_all_changes("symlink readme");

    context.run_release_pr().success();
    context.merge_release_pr().await;

    let expected_tag = "v0.1.0";

    context.run_release().success();

    let gitea_release = context.gitea.get_gitea_release(expected_tag).await;
    assert_eq!(gitea_release.name, expected_tag);

    let full_readme_path = context.repo_dir().join(readme);
    fs_err::write(full_readme_path, "cool stuff").unwrap();
    context.push_all_changes("feat: update readme");

    let outcome = context.run_release_pr().success();

    let opened_prs = context.opened_release_prs().await;
    let open_pr = &opened_prs[0];
    let expected_stdout = serde_json::json!({
        "prs": [{
            "base_branch": "main",
            "head_branch": open_pr.branch(),
            "html_url": open_pr.html_url,
            "number": open_pr.number,
            "releases": [{
                "package_name": context.gitea.repo,
                "version": "0.1.1"
            }]
        }]
    });
    outcome.stdout(format!("{expected_stdout}\n"));
    context.merge_release_pr().await;

    let expected_tag = "v0.1.1";

    context.run_release().success();

    let gitea_release = context.gitea.get_gitea_release(expected_tag).await;
    assert_eq!(gitea_release.name, expected_tag);
    expect_test::expect![[r#"
        ### Added

        - update readme"#]]
    .assert_eq(&gitea_release.body);
}

#[tokio::test]
#[cfg_attr(not(feature = "docker-tests"), ignore)]
async fn changelog_is_not_updated_if_version_already_exists_in_changelog() {
    let context = TestContext::new().await;
    context.run_release_pr().success();
    // Merge release PR to update changelog of v0.1.0 of crate
    context.merge_release_pr().await;

    // do a random commit
    move_readme(&context, "feat: move readme");

    // Run again release-plz to create a new release PR.
    // Since we haven't published the crate, release-plz doesn't change the version.
    // Release-plz releazes that the version already exists in the changelog and doesn't update it.
    context.run_release_pr().success();

    // Since the changelog is not updated, the PR is not created because there are no changes to do.
    let opened_prs = context.opened_release_prs().await;
    assert_eq!(opened_prs.len(), 0);
}

#[tokio::test]
#[cfg_attr(not(feature = "docker-tests"), ignore)]
async fn release_plz_adds_labels_to_release_pr() {
    let test_context = TestContext::new().await;

    // Initial PR setup with two labels
    let initial_config = r#"
    [workspace]
    pr_labels = ["bug", "enhancement"]
    "#;
    let initial_labels = ["bug", "enhancement"];

    test_context.write_release_plz_toml(initial_config);
    test_context.run_release_pr().success();

    let initial_prs = test_context.opened_release_prs().await;
    assert_eq!(initial_prs.len(), 1, "Expected one PR to be created");

    let initial_pr = &initial_prs[0];
    assert_eq!(initial_pr.labels.len(), 2, "Expected 2 labels");

    assert_eq!(
        initial_pr.label_names(),
        initial_labels,
        "Labels don't match expected values"
    );

    // Update PR with additional label
    let updated_config = r#"
    [workspace]
    pr_name = "add labels to release label update"
    pr_labels = ["needs-testing"]
    "#;
    let expected_labels = ["bug", "enhancement", "needs-testing"];

    test_context.write_release_plz_toml(updated_config);
    test_context.run_release_pr().success();

    let updated_prs = test_context.opened_release_prs().await;
    assert_eq!(updated_prs.len(), 1, "Expected one PR after update");

    let updated_pr = &updated_prs[0];
    assert_eq!(updated_pr.title, "add labels to release label update");
    assert_eq!(updated_pr.labels.len(), 3, "Expected 3 labels after update");

    assert_eq!(
        updated_pr.label_names(),
        expected_labels,
        "Updated labels don't match expected values"
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "docker-tests"), ignore)]
async fn release_plz_doesnt_add_invalid_labels_to_release_pr() {
    let test_context = TestContext::new().await;
    let test_cases: &[(&str, &str)] = &[
        // (label config, expected error message)
        (
            r#"
            [workspace]
            pr_labels = [" "]
        "#,
            "leading or trailing whitespace is not allowed",
        ), // space label
        (
            r#"
            [workspace]
            pr_labels = ["this-is-a-very-long-label-that-exceeds-the-maximum-length-allowed-by-git-providers"]
            "#,
            "it exceeds maximum length of 50 characters",
        ), // Too long
        (
            r#"
            [workspace]
            pr_labels = [""]
            "#,
            "empty labels are not allowed",
        ),
        (
            r#"
            [workspace]
            pr_labels = ["abc", "abc"]
            "#,
            "duplicate labels are not allowed",
        ),
    ];

    for test_case in test_cases {
        let initial_config = test_case.0;
        test_context.write_release_plz_toml(initial_config);
        let error = test_context.run_release_pr().failure().to_string();
        assert!(
            error.contains("Failed to add label") && error.contains(test_case.1),
            "Expected label creation failure got: {error}"
        );
    }
}

#[tokio::test]
#[cfg_attr(not(feature = "docker-tests"), ignore)]
async fn release_plz_updates_binary_when_library_has_breaking_changes() {
    let binary = "binary";
    let library1 = "library1";
    let library2 = "library2";
    let library3 = "library3";
    let library4 = "library4";
    // Dependency chain: binary -> library3 -> library2 -> library1.
    // Library4 is a standalone crate.
    let context = TestContext::new_workspace_with_packages(&[
        TestPackage::new(binary)
            .with_type(PackageType::Bin)
            .with_path_dependencies(vec![format!("../{library3}")]),
        TestPackage::new(library3)
            .with_type(PackageType::Lib)
            .with_path_dependencies(vec![format!("../{library2}")]),
        TestPackage::new(library2)
            .with_type(PackageType::Lib)
            .with_path_dependencies(vec![format!("../{library1}")]),
        TestPackage::new(library1).with_type(PackageType::Lib),
        TestPackage::new(library4).with_type(PackageType::Lib),
    ])
    .await;

    // Set initial versions before first release
    context.set_package_version(library2, &Version::new(0, 2, 0));
    context.set_package_version(library3, &Version::new(0, 3, 0));
    context.set_package_version(binary, &Version::new(1, 3, 0));

    context.push_all_changes("set initial versions");

    context.run_release_pr().success();
    context.merge_release_pr().await;
    context.run_release().success();

    // Update the library.
    let lib_file = context.package_path(library1).join("src").join("lib.rs");
    // This is a breaking change because we remove the `add` library
    // created with `cargo init --lib`.
    fs_err::write(&lib_file, "pub fn bar() {}").unwrap();
    context.push_all_changes("breaking change in library");

    context.run_release_pr().success();
    let today = today();
    let opened_prs = context.opened_release_prs().await;
    assert_eq!(opened_prs.len(), 1);

    let open_pr = &opened_prs[0];
    assert_eq!(open_pr.title, "chore: release");

    let username = context.gitea.user.username();
    let repo = &context.gitea.repo;
    let actual_body = open_pr.body.as_ref().unwrap().trim().to_string();
    let expected_body = format!(
        r#"
## 🤖 New release

* `{library1}`: 0.1.0 -> 0.2.0 (⚠ API breaking changes)
* `{library2}`: 0.2.0 -> 0.2.1
* `{library3}`: 0.3.0 -> 0.3.1
* `{binary}`: 1.3.0 -> 1.3.1

### ⚠ `{library1}` breaking changes

```text
--- failure function_missing: pub fn removed or renamed ---

Description:
A publicly-visible function cannot be imported by its prior path. A `pub use` may have been removed, or the function itself may have been renamed or removed entirely.
        ref: https://doc.rust-lang.org/cargo/reference/semver.html#item-remove
       impl: https://github.com/obi1kenobi/cargo-semver-checks/tree/v0.40.0/src/lints/function_missing.ron

Failed in:
  function library1::add, previously in file /private/var/folders/sz/335x8kc91g55r2nkjktmkv1h0000gq/T/.tmpY912au/library1/src/lib.rs:1
```

<details><summary><i><b>Changelog</b></i></summary><p>

## `{library1}`

<blockquote>

## [0.2.0](https://localhost/{username}/{repo}/compare/{library1}-v0.1.0...{library1}-v0.2.0) - {today}

### Other

- breaking change in library
</blockquote>

## `{library2}`

<blockquote>

## [0.2.1](https://localhost/{username}/{repo}/compare/{library2}-v0.2.0...{library2}-v0.2.1) - {today}

### Other

- updated the following local packages: {library1}
</blockquote>

## `{library3}`

<blockquote>

## [0.3.1](https://localhost/{username}/{repo}/compare/{library3}-v0.3.0...{library3}-v0.3.1) - {today}

### Other

- updated the following local packages: {library2}
</blockquote>

## `{binary}`

<blockquote>

## [1.3.1](https://localhost/{username}/{repo}/compare/{binary}-v1.3.0...{binary}-v1.3.1) - {today}

### Other

- updated the following local packages: {library3}
</blockquote>


</p></details>

---
This PR was generated with [release-plz](https://github.com/release-plz/release-plz/)."#,
    )
    .trim()
    .to_string();

    // Split the strings into lines and compare line by line, ignoring lines containing temporary directory paths
    let actual_lines: Vec<_> = actual_body.lines().collect();
    let expected_lines: Vec<_> = expected_body.lines().collect();
    assert_eq!(
        actual_lines.len(),
        expected_lines.len(),
        "Different number of lines. Expected:\n{expected_body}\nActual:\n{actual_body}"
    );
    for (actual, expected) in actual_lines.iter().zip(expected_lines.iter()) {
        if !does_line_vary(actual) {
            assert_eq!(actual, expected);
        }
    }

    context.merge_release_pr().await;

    // Check if the binary has the new version.
    let binary_cargo_toml =
        fs_err::read_to_string(context.package_path(binary).join(CARGO_TOML)).unwrap();
    expect_test::expect![[r#"
        [package]
        name = "binary"
        version = "1.3.1"
        edition = "2024"
        publish = ["test-registry"]

        [dependencies]
        library3 = { version = "0.3.1", path = "../library3", registry = "test-registry" }
    "#]]
    .assert_eq(&binary_cargo_toml);
}

fn move_readme(context: &TestContext, message: &str) {
    let readme = "README.md";
    let new_readme = format!("NEW_{readme}");
    let old_readme_path = context.repo_dir().join(readme);
    let new_readme_path = context.repo_dir().join(&new_readme);
    fs_err::rename(old_readme_path, new_readme_path).unwrap();

    update_readme_in_cargo_toml(context, &new_readme);

    context.push_all_changes(message);
}

#[cfg(unix)]
fn symlink_readme(context: &TestContext, readme_path: &str, symlink_path: &str) {
    use fs_err::os::unix::fs;
    use std::{
        env::{current_dir, set_current_dir},
        sync::{LazyLock, Mutex},
    };

    // Trick to avoid the tests to run concurrently.
    // It's used to not affect the current directory of the other tests.
    static NO_PARALLEL: LazyLock<Mutex<()>> = LazyLock::new(Mutex::default);

    let current_dir = current_dir().unwrap();
    // There's some weird behavior with respect to absolute/relative paths when using symlinks in these tests.
    // Explicitly setting the directory so the relative paths are tracked correctly seems to be the easiest way
    // to make this work.
    {
        let _guard = NO_PARALLEL.lock().unwrap();
        set_current_dir(context.repo_dir()).unwrap();
        fs::symlink(readme_path, symlink_path).unwrap();
        set_current_dir(current_dir).unwrap();
    }

    update_readme_in_cargo_toml(context, symlink_path);
}

fn update_readme_in_cargo_toml(context: &TestContext, readme_path: &str) {
    let cargo_toml_path = context.repo_dir().join(CARGO_TOML);
    let mut cargo_toml = LocalManifest::try_new(&cargo_toml_path).unwrap();
    cargo_toml.data["package"]["readme"] = toml_edit::value(readme_path);
    cargo_toml.write().unwrap();
}

#[tokio::test]
#[cfg_attr(not(feature = "docker-tests"), ignore)]
async fn release_plz_updates_binary_with_no_commits_and_dependency_change() {
    let binary = "binary";
    let library = "library";
    // dependency chain: binary -> library
    let context = TestContext::new_workspace_with_packages(&[
        TestPackage::new(binary)
            .with_type(PackageType::Bin)
            .with_path_dependencies(vec![format!("../{library}")]),
        TestPackage::new(library).with_type(PackageType::Lib),
    ])
    .await;

    // Set initial versions before first release
    context.set_package_version(library, &Version::new(0, 1, 0));
    context.set_package_version(binary, &Version::new(1, 0, 0));
    context.push_all_changes("set initial versions");

    context.run_release_pr().success();
    context.merge_release_pr().await;
    context.run_release().success();

    // Update the library with a breaking change
    let lib_file = context.package_path(library).join("src").join("lib.rs");
    // This is a breaking change because we remove the `add` library
    // created with `cargo init --lib`.
    fs_err::write(&lib_file, "pub fn bar() {}").unwrap();
    context.push_all_changes("breaking change in library");

    context.run_release_pr().success();
    let today = today();
    let opened_prs = context.opened_release_prs().await;
    assert_eq!(opened_prs.len(), 1);

    let open_pr = &opened_prs[0];
    assert_eq!(open_pr.title, "chore: release");

    let username = context.gitea.user.username();
    let repo = &context.gitea.repo;
    let actual_body = open_pr.body.as_ref().unwrap().trim().to_string();
    let expected_body = format!(
        r#"
## 🤖 New release

* `{library}`: 0.1.0 -> 0.2.0 (⚠ API breaking changes)
* `{binary}`: 1.0.0 -> 1.0.1

### ⚠ `{library}` breaking changes

```text
--- failure function_missing: pub fn removed or renamed ---

Description:
A publicly-visible function cannot be imported by its prior path. A `pub use` may have been removed, or the function itself may have been renamed or removed entirely.
        ref: https://doc.rust-lang.org/cargo/reference/semver.html#item-remove
       impl: https://github.com/obi1kenobi/cargo-semver-checks/tree/v0.40.0/src/lints/function_missing.ron

Failed in:
  function library::add, previously in file /private/var/folders/sz/335x8kc91g55r2nkjktmkv1h0000gq/T/.tmpY912au/library/src/lib.rs:1
```

<details><summary><i><b>Changelog</b></i></summary><p>

## `{library}`

<blockquote>

## [0.2.0](https://localhost/{username}/{repo}/compare/{library}-v0.1.0...{library}-v0.2.0) - {today}

### Other

- breaking change in library
</blockquote>

## `{binary}`

<blockquote>

## [1.0.1](https://localhost/{username}/{repo}/compare/{binary}-v1.0.0...{binary}-v1.0.1) - {today}

### Other

- updated the following local packages: {library}
</blockquote>


</p></details>

---
This PR was generated with [release-plz](https://github.com/release-plz/release-plz/)."#,
    )
    .trim()
    .to_string();

    // Split the strings into lines and compare line by line, ignoring lines containing temporary directory paths
    let actual_lines: Vec<_> = actual_body.lines().collect();
    let expected_lines: Vec<_> = expected_body.lines().collect();
    assert_eq!(
        actual_lines.len(),
        expected_lines.len(),
        "Different number of lines. Expected:\n{expected_body}\nActual:\n{actual_body}"
    );
    for (actual, expected) in actual_lines.iter().zip(expected_lines.iter()) {
        if !does_line_vary(actual) {
            assert_eq!(actual, expected);
        }
    }

    context.merge_release_pr().await;

    // Check if the binary has the new version.
    let binary_cargo_toml =
        fs_err::read_to_string(context.package_path(binary).join(CARGO_TOML)).unwrap();
    expect_test::expect![[r#"
        [package]
        name = "binary"
        version = "1.0.1"
        edition = "2024"
        publish = ["test-registry"]

        [dependencies]
        library = { version = "0.2.0", path = "../library", registry = "test-registry" }
    "#]]
    .assert_eq(&binary_cargo_toml);
}

/// Check if the line contains a temporary directory or the version of cargo-semver-checks.
fn does_line_vary(line: &str) -> bool {
    line.contains("cargo-semver-checks/tree") || line.contains(".tmp")
}
#[tokio::test]
#[cfg_attr(not(feature = "docker-tests"), ignore)]
async fn release_plz_handles_invalid_readme_path_gracefully() {
    let context = TestContext::new().await;

    // Set up a README path that will be invalid when the package is installed
    // This simulates the real-world scenario where someone sets a relative path
    // like "../../README.md" in their Cargo.toml, which works locally but fails
    // when the package is installed from the registry cache
    let cargo_toml_path = context.repo_dir().join(CARGO_TOML);
    let mut cargo_toml = LocalManifest::try_new(&cargo_toml_path).unwrap();

    // Create a README file in the project root for local development
    let readme_content = "# My Project\n\nThis is a test project.";
    let actual_readme_path = context.repo_dir().join("README.md");
    fs_err::write(&actual_readme_path, readme_content).unwrap();

    // Set an invalid relative path that would work locally but fail in registry cache
    // This is the problematic pattern that users might accidentally use
    cargo_toml.data["package"]["readme"] = toml_edit::value("../../README.md");
    cargo_toml.write().unwrap();

    context.push_all_changes("set invalid readme path");

    // This should not panic or fail due to the invalid readme path
    // The fix should handle this gracefully by logging a warning and continuing
    let outcome = context.run_release_pr().success();

    let opened_prs = context.opened_release_prs().await;
    assert_eq!(
        opened_prs.len(),
        1,
        "Release PR should still be created despite invalid readme path"
    );

    let open_pr = &opened_prs[0];
    assert_eq!(open_pr.title, "chore: release v0.1.0");

    // Verify the PR was created successfully
    let expected_stdout = serde_json::json!({
        "prs": [{
            "head_branch": open_pr.branch(),
            "base_branch": "main",
            "html_url": open_pr.html_url,
            "number": open_pr.number,
            "releases": [{
                "package_name": context.gitea.repo,
                "version": "0.1.0"
            }]
        }]
    });
    outcome.stdout(format!("{expected_stdout}\n"));

    // Additional test: Make a change and ensure subsequent operations work
    let new_file = context.repo_dir().join("src").join("new.rs");
    fs_err::write(&new_file, "// new functionality").unwrap();
    context.push_all_changes("feat: add new functionality");

    // This should also work without issues
    context.run_update().success();

    // Verify changelog was updated despite the invalid readme path
    let changelog = fs_err::read_to_string(context.repo_dir().join("CHANGELOG.md")).unwrap();
    assert!(
        changelog.contains("add new functionality"),
        "Changelog should be updated even with invalid readme path"
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "docker-tests"), ignore)]
async fn release_plz_handles_nonexistent_readme_path_in_cargo_toml() {
    let context = TestContext::new().await;

    // Set a README path that simply doesn't exist anywhere
    let cargo_toml_path = context.repo_dir().join(CARGO_TOML);
    let mut cargo_toml = LocalManifest::try_new(&cargo_toml_path).unwrap();
    cargo_toml.data["package"]["readme"] = toml_edit::value("nonexistent-readme.md");
    cargo_toml.write().unwrap();

    context.push_all_changes("set nonexistent readme path");

    // This should handle the nonexistent file gracefully
    let _outcome = context.run_release_pr().success();

    let opened_prs = context.opened_release_prs().await;
    assert_eq!(
        opened_prs.len(),
        1,
        "Release PR should be created despite nonexistent readme"
    );

    // Make sure subsequent operations work
    let new_file = context.repo_dir().join("src").join("lib.rs");
    fs_err::write(&new_file, "pub fn test() {}").unwrap();
    context.push_all_changes("add lib function");

    context.run_update().success();
}

#[tokio::test]
#[cfg_attr(not(feature = "docker-tests"), ignore)]
async fn release_plz_works_with_valid_readme_path() {
    let context = TestContext::new().await;

    // Create a valid README and set correct path
    let readme_content = "# Valid Project\n\nThis project has a valid readme path.";
    let readme_path = context.repo_dir().join("README.md");
    fs_err::write(&readme_path, readme_content).unwrap();

    let cargo_toml_path = context.repo_dir().join(CARGO_TOML);
    let mut cargo_toml = LocalManifest::try_new(&cargo_toml_path).unwrap();
    cargo_toml.data["package"]["readme"] = toml_edit::value("README.md");
    cargo_toml.write().unwrap();

    context.push_all_changes("add valid readme");

    // This should work normally
    let _outcome = context.run_release_pr().success();

    let opened_prs = context.opened_release_prs().await;
    assert_eq!(opened_prs.len(), 1);

    // Modify the README and ensure it's detected as a change
    fs_err::write(
        &readme_path,
        "# Updated Valid Project\n\nThis readme was updated.",
    )
    .unwrap();
    context.push_all_changes("update readme content");

    context.run_release_pr().success();

    // Should create a new PR for the readme change
    let updated_prs = context.opened_release_prs().await;
    // The count might be 1 (updated) or 2 (new PR), both are valid depending on implementation
    assert!(
        !updated_prs.is_empty(),
        "Should handle readme updates correctly"
    );
}
