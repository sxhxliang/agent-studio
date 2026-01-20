# Contributing to AgentX

Thank you for your interest in contributing to AgentX! We welcome contributions from the community and are grateful for your support.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [How to Contribute](#how-to-contribute)
- [Code Style Guidelines](#code-style-guidelines)
- [Commit Message Guidelines](#commit-message-guidelines)
- [Pull Request Process](#pull-request-process)
- [Testing](#testing)
- [Documentation](#documentation)
- [Community](#community)

---

## Code of Conduct

By participating in this project, you agree to maintain a respectful and inclusive environment for everyone. Be kind, be professional, and be collaborative.

### Our Standards

- **Be respectful**: Treat everyone with respect and consideration
- **Be constructive**: Provide constructive feedback and suggestions
- **Be patient**: Help others learn and grow
- **Be inclusive**: Welcome diverse perspectives and backgrounds

---

## Getting Started

### Prerequisites

Before contributing, ensure you have:

- **Rust 1.83+** (2024 edition)
- **Git** for version control
- **Platform-specific dependencies**:
  - **macOS**: Xcode command line tools
  - **Linux**: `libxcb`, `libfontconfig`, `libssl-dev`
  - **Windows**: MSVC toolchain

### Finding Issues to Work On

1. Browse the [issue tracker](https://github.com/sxhxliang/gpui-component/issues)
2. Look for issues labeled:
   - `good first issue` - Great for newcomers
   - `help wanted` - We need assistance
   - `bug` - Something isn't working
   - `enhancement` - New feature or request

3. Comment on the issue to let others know you're working on it

---

## Development Setup

### 1. Fork and Clone

```bash
# Fork the repository on GitHub first

# Clone your fork
git clone https://github.com/YOUR-USERNAME/gpui-component.git
cd gpui-component/agent-studio
```

### 2. Build the Project

```bash
# Debug build
cargo build

# Run the application
cargo run

# Run with logging
RUST_LOG=info cargo run
```

### 3. Create a Branch

```bash
# Create a feature branch
git checkout -b feature/your-feature-name

# Or for bug fixes
git checkout -b fix/bug-description
```

---

## How to Contribute

### Reporting Bugs

**Before submitting**, please:
1. Check if the bug has already been reported
2. Test with the latest version
3. Verify it's reproducible

**Bug Report Template:**

```markdown
### Description
Clear description of the bug

### Steps to Reproduce
1. Step one
2. Step two
3. ...

### Expected Behavior
What should happen

### Actual Behavior
What actually happens

### Environment
- OS: [e.g., macOS 14.0]
- AgentX Version: [e.g., 0.5.0]
- Rust Version: [e.g., 1.83]

### Additional Context
Screenshots, logs, or other relevant information
```

### Suggesting Features

**Feature Request Template:**

```markdown
### Problem
What problem does this feature solve?

### Proposed Solution
How would this feature work?

### Alternatives Considered
Other approaches you've thought about

### Additional Context
Mockups, examples, or use cases
```

### Contributing Code

1. **Discuss first** for large changes
2. **Keep it focused** - One feature/fix per PR
3. **Write tests** when applicable
4. **Update documentation** as needed
5. **Follow code style** guidelines

---

## Code Style Guidelines

### Rust Style

We follow standard Rust conventions:

```rust
// Use descriptive names
fn calculate_total_price(items: &[Item]) -> f64 {
    items.iter().map(|item| item.price).sum()
}

// Group imports logically
use std::collections::HashMap;

use anyhow::Result;
use gpui::{App, Context, Entity};

use crate::core::AppState;
use super::panel::Panel;

// Document public APIs
/// Calculates the total price of all items
///
/// # Arguments
/// * `items` - A slice of items to sum
///
/// # Returns
/// The total price as a float
pub fn calculate_total(items: &[Item]) -> f64 {
    // implementation
}
```

### GPUI Patterns

**Entity Lifecycle:**

```rust
// ❌ WRONG - Entity dies after render
fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let widget = cx.new(|cx| Widget::new()); // Dies!
    v_flex().child(widget)
}

// ✅ CORRECT - Store in struct
struct MyPanel {
    widget: Entity<Widget>,
}

fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    v_flex().child(self.widget.clone())
}
```

**Event Bus Pattern:**

```rust
// Subscribe in UI thread
let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
session_bus.subscribe(move |event| {
    let _ = tx.send((*event.update).clone());
});

cx.spawn(|mut cx| async move {
    while let Some(update) = rx.recv().await {
        cx.update(|cx| {
            // Update UI
            cx.notify();
        });
    }
}).detach();
```

### Formatting

```bash
# Format all code
cargo fmt

# Check formatting
cargo fmt --check

# Lint with Clippy
cargo clippy -- --deny warnings
```

---

## Commit Message Guidelines

We follow **Conventional Commits** for clear history:

### Format

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Types

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `perf`: Performance improvements
- `test`: Adding or updating tests
- `chore`: Maintenance tasks
- `ci`: CI/CD changes

### Examples

```bash
feat(conversation): add markdown rendering support

Implements CommonMark-compliant markdown rendering in chat messages.
Includes support for code blocks with syntax highlighting.

Closes #123

---

fix(terminal): resolve shell path detection on Windows

The terminal failed to find PowerShell on Windows 11.
Now checks multiple common paths.

Fixes #456

---

docs(readme): update installation instructions

Added instructions for Linux package managers.
```

### Rules

- Use **present tense**: "add feature" not "added feature"
- Use **imperative mood**: "fix bug" not "fixes bug"
- **Capitalize** first letter of subject
- **No period** at end of subject
- Keep subject line **under 72 characters**
- Reference issues/PRs in footer

---

## Pull Request Process

### Before Submitting

1. **Update from main**:
   ```bash
   git checkout main
   git pull upstream main
   git checkout your-branch
   git rebase main
   ```

2. **Run checks**:
   ```bash
   cargo fmt --check
   cargo clippy -- --deny warnings
   cargo test
   cargo build --release
   ```

3. **Update documentation** if needed

### PR Template

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Changes Made
- Change 1
- Change 2
- ...

## Testing
How was this tested?

## Screenshots (if applicable)
Add screenshots for UI changes

## Checklist
- [ ] Code follows project style
- [ ] Tests added/updated
- [ ] Documentation updated
- [ ] All tests pass
- [ ] No new warnings
```

### Review Process

1. **Automated checks** must pass
2. At least **one maintainer** will review
3. Address feedback in new commits
4. Once approved, maintainer will merge

### After Merging

1. Delete your branch
2. Pull latest changes
3. Celebrate! 🎉

---

## Testing

### Running Tests

```bash
# Run all tests
cargo test --all

# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture

# Run integration tests
cargo test --test integration_test
```

### Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature() {
        // Arrange
        let input = create_test_data();

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected);
    }

    #[test]
    #[should_panic(expected = "error message")]
    fn test_error_handling() {
        panic_function();
    }
}
```

### Test Guidelines

- Test **public APIs**, not private implementation
- Use **descriptive test names**
- Follow **Arrange-Act-Assert** pattern
- Test **edge cases** and error conditions
- Keep tests **fast** and **deterministic**

---

## Documentation

### Code Documentation

```rust
/// Brief one-line description
///
/// Longer description with more details.
///
/// # Arguments
/// * `param` - Description of parameter
///
/// # Returns
/// Description of return value
///
/// # Examples
/// ```
/// let result = function(42);
/// assert_eq!(result, expected);
/// ```
///
/// # Panics
/// Conditions that cause panics
///
/// # Errors
/// Error conditions and meanings
pub fn function(param: i32) -> Result<String> {
    // implementation
}
```

### User Documentation

- Update README for user-facing changes
- Add examples to docs folder
- Include screenshots for UI changes
- Update changelog

---

## Community

### Getting Help

- **GitHub Discussions**: Ask questions, share ideas
- **Issues**: Report bugs, request features
- **Pull Requests**: Contribute code

### Recognition

Contributors are recognized in:
- README acknowledgments
- Release notes
- GitHub contributors page

Thank you for contributing to AgentX! Your efforts make this project better for everyone. 🚀

---

## License

By contributing, you agree that your contributions will be licensed under the Apache-2.0 License.
