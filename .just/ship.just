# Release a new version, pass --help for options to `uv version --bump`
[working-directory: 'polars-genson-py']
release bump_level="patch":
    #!/usr/bin/env -S echo-comment --shell-flags="-e" --color bright-green
    
    ## Exit early if help was requested
    if [[ "{{bump_level}}" == "--help" ]]; then
        uv version --help
        exit 0
    fi

    just check-no-fmt-feat
    
    # 📈 Bump the version in pyproject.toml (patch/minor/major: {{bump_level}})
    uv version --frozen --bump {{bump_level}}

    # 🦀 Bump each changed crate's version and write its changelog (release-plz, from the
    # commits since the crate's last release), in this release commit: CI publishes them
    # to crates.io (the crates job in CI.yml)
    (cd .. && release-plz update --allow-dirty)
    
    # 📦 Stage all changes (including the version bump)
    git add --all
    
    # 🔄 Create a temporary commit to capture the new version
    git commit -m "chore(temp): version check"
     
    # ✂️  Extract the new version number that was just set, undo the commit
    new_version=$(uv version --short)
    git reset --soft HEAD~1
     
    # ✅ Stage everything again and create the real release commit
    git add --all
    git commit -m  "chore(release): bump 🐍 -> v$new_version"
     
    # 🏷️ Create the git tag for this release
    git tag -a "py-$new_version" -m "Python Release $new_version"
    
    branch_name=$(git rev-parse --abbrev-ref HEAD);
    # 🚀 Push the release commit to $branch_name
    git push origin $branch_name
    
    # 🚀 Push the commit tag to the remote
    git push origin "py-$new_version"
