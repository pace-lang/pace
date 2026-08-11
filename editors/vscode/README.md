# Pace Language Support

This is the official Visual Studio Code extension for the **Pace** programming language.

## Features

- **Rich Syntax Highlighting**: Comprehensive coloring for keywords, types, strings, comments, classes, and Enums.
- **Auto-Closing & Bracket Matching**: Intelligently toggles block comments (`/* */`), line comments (`//`), and auto-completes brackets `{}`, `()`, `[]`.
- **Snippets**: Powerful boilerplate templates. Simply type the following and hit `Enter` or `Tab`:
  - `func` -> Expands to a function definition.
  - `class` -> Expands to a class definition with an initializer.
  - `enum` -> Expands to an Enum.
  - `match` -> Expands to a pattern matching block.
  - `main` -> Expands to the standard entry point.
- **LSP Skeleton Setup**: Pre-configured Language Server Protocol setup (ready to connect to the Pace compiler `lsp` command for future diagnostics and autocompletion).

## Installation

1. Download the latest `pace-lang-x.x.x.vsix` release.
2. Open VS Code.
3. Open the Extensions view (`Ctrl+Shift+X` or `Cmd+Shift+X`).
4. Click the `...` menu in the top right and select **Install from VSIX...**.
5. Select the downloaded file.

## Requirements

To use advanced features in the future (like live compiler errors), ensure you have the `pace` compiler installed and available in your system `PATH`.

## License

This extension is licensed under the [MIT License](LICENSE).
