# Contributing guidelines

Thank you for wanting to contribute to this project!

## Contributing code

There are a couple of things we are going to look out for in PRs and knowing them upfront is going to reduce the number of times we will be going back and forth, making things more efficient.

1. We have CI checks in place that validate formatting and code style.
   Make sure `just fmt` and `just lint` both finish without any warnings or errors on every commit.
   - If you don't already have `just` installed, you can obtain it by calling `cargo install just`.
   - If you don't already have `dprint` installed (utility used for code formatting), you can obtain it [here](https://dprint.dev/install/).
2. All text documents (`CHANGELOG.md`, `README.md`, etc) should follow the [semantic linebreaks](https://sembr.org/) specification.
3. We strive for atomic commits with good commit messages.
   As an inspiration, read [this](https://chris.beams.io/posts/git-commit/) blogpost.
   An atomic commit is a cohesive diff with formatting checks, linter and build passing.
   Ideally, all tests are passing as well but we acknowledge that this is not always possible depending on the change you are making.
4. If you are making any user-facing changes, include a changelog entry.

## Contributing issues

When adding a bug report or feature request, please focus on your _problem_ as much as possible.

The provided issue templates should be helpful in structuring an issue in a way that can make it actionable by the developers.

It is okay to include ideas on how the feature could be implemented but they shouldn't be the focus of your request.

For more loosely-defined problems and ideas, consider starting a [discussion](https://github.com/get10101/10101/discussions/new) instead of opening an issue.
