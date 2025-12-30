# Slash Command Autocomplete Feature

## Overview

The WelcomePanel now supports slash command autocomplete. When you type `/` in the input box and have an active session with available commands, the system will display a filtered list of matching commands with their descriptions.

## How It Works

### User Experience

1. **Create or Select a Session**:
   - Select an agent from the dropdown
   - Either select an existing session or create a new one by clicking the "+" button

2. **Trigger Command Autocomplete**:
   - Type `/` at the beginning of the input box
   - The system will instantly display all available commands for the current session

3. **Filter Commands**:
   - Continue typing after the `/` to filter commands
   - For example, typing `/re` will show commands like `/review`, `/reload`, etc.
   - The filtering is prefix-based and case-sensitive

4. **View Command Details**:
   - Each command is displayed with:
     - Command name (in accent color with `/` prefix)
     - Description (if available)

5. **Hide Suggestions**:
   - Delete the `/` or type other characters to hide the suggestions
   - The suggestions disappear automatically when input doesn't start with `/`

## Implementation Details

### Architecture

```
User types "/" → WelcomePanel.on_input_change()
  ├─→ get_available_commands(cx)
  │    └─→ MessageService.get_commands_by_session_id()
  │         └─→ AgentService.get_session_commands()
  │              └─→ Returns Vec<AvailableCommand>
  ├─→ Filter commands by prefix
  │    └─→ Update self.command_suggestions
  └─→ ChatInputBox.render()
       └─→ Display command suggestions list
```

### Data Flow

1. **Command Storage**:
   - When agent sends `AvailableCommandsUpdate`, it's automatically stored in `AgentSessionInfo.available_commands`
   - This happens via `MessageService.init_persistence()` subscription

2. **Command Retrieval**:
   - WelcomePanel calls `get_available_commands()` which queries the current session ID
   - MessageService provides convenient `get_commands_by_session_id()` method

3. **UI Update**:
   - Filtered commands are passed to ChatInputBox via `.command_suggestions()`
   - ChatInputBox renders them in a styled list below the input area

### Code Locations

**WelcomePanel** (`src/panels/welcome_panel.rs`):
- `command_suggestions: Vec<AvailableCommand>` - Filtered command list
- `show_command_suggestions: bool` - Whether to display suggestions
- `on_input_change()` - Detects `/` and filters commands
- `get_available_commands()` - Queries commands for current session

**ChatInputBox** (`src/components/chat_input_box.rs`):
- `command_suggestions()` - Builder method to set commands
- `show_command_suggestions()` - Builder method to control visibility
- Command suggestions rendered in `v_flex()` with styled command items

## Features

### Visual Design

The command suggestions list:
- **Container**: Rounded box with muted background and border
- **Header**: "Available Commands:" label in small, semibold text
- **Command Items**:
  - Hover effect with secondary background
  - Command name in accent color with medium weight font
  - Description in foreground color
  - Responsive layout with proper spacing

### Filtering Logic

```rust
// Show all commands when just "/" is entered
if command_prefix.is_empty() {
    self.command_suggestions = all_commands;
}
// Filter by prefix
else {
    self.command_suggestions = all_commands
        .into_iter()
        .filter(|cmd| cmd.name.starts_with(command_prefix))
        .collect();
}
```

### Session Requirement

- Commands are only shown when a session is active (`current_session_id.is_some()`)
- If no session exists, an empty list is returned
- This ensures commands are always contextual to the selected agent

## Examples

### Example 1: Show All Commands

**Input**: `/`

**Display**:
```
Available Commands:
/compact    - Clear conversation history but keep a summary in context
/init       - Initialize a new CLAUDE.md file with codebase documentation
/pr-comments - Get comments from a GitHub pull request
/review     - Review a pull request
/security-review - Complete a security review of the pending changes
```

### Example 2: Filter Commands

**Input**: `/re`

**Display**:
```
Available Commands:
/review     - Review a pull request
```

### Example 3: No Matches

**Input**: `/xyz`

**Display**: (No suggestions shown - list is hidden)

## Logging

Debug logs are generated for command filtering:

```
[DEBUG agentx::panels::welcome_panel] Command suggestions: 5 matches for prefix ''
[DEBUG agentx::panels::welcome_panel] Command suggestions: 1 matches for prefix 're'
```

To enable:
```bash
RUST_LOG=info,agentx::panels::welcome_panel=debug cargo run
```

## Integration with Other Features

### File Picker (`@` mention)

- When `@` is typed, file picker takes precedence
- Command suggestions are hidden (`show_command_suggestions = false`)
- This prevents UI conflicts between the two autocomplete features

### Paste Handler

- Pasting content doesn't trigger command suggestions
- Only direct typing of `/` activates the feature

### Session Management

- Switching agents or sessions automatically updates available commands
- Commands are fetched in real-time from the session's `AvailableCommand` list

## Future Enhancements

Potential improvements:

1. **Click to Insert**: Allow clicking on a command to insert it into the input
2. **Keyboard Navigation**: Arrow keys to navigate suggestions, Enter to select
3. **Command Arguments**: Show expected arguments for each command
4. **Fuzzy Matching**: Support fuzzy search instead of just prefix matching
5. **Command History**: Remember recently used commands
6. **Command Grouping**: Group commands by category (e.g., git, review, init)
7. **Rich Tooltips**: Show detailed help on hover

## API Reference

### WelcomePanel Methods

```rust
/// Handle input changes and detect / for command suggestions
fn on_input_change(&mut self, cx: &mut Context<Self>)

/// Get available commands for the current session
fn get_available_commands(&self, cx: &Context<Self>) -> Vec<AvailableCommand>
```

### ChatInputBox Builder Methods

```rust
/// Set command suggestions to display
pub fn command_suggestions(mut self, commands: Vec<AvailableCommand>) -> Self

/// Set whether to show command suggestions
pub fn show_command_suggestions(mut self, show: bool) -> Self
```

## Testing

To test the feature:

1. Run the application:
   ```bash
   cargo run
   ```

2. In WelcomePanel:
   - Select "Claude Code" agent
   - Click "+" to create a new session
   - Wait for the session to initialize (AvailableCommandsUpdate event)

3. Type `/` in the input box

4. Observe the command list appearing below the input

5. Type additional characters to filter (e.g., `/com`, `/rev`)

6. Delete characters or type non-slash text to hide suggestions

## Troubleshooting

**Issue**: No commands shown when typing `/`

**Possible Causes**:
- No active session (`current_session_id` is None)
- Session hasn't received `AvailableCommandsUpdate` yet
- MessageService not initialized
- Agent doesn't support commands

**Solution**:
- Check logs for "No current session, cannot get commands"
- Wait a moment after creating session
- Verify agent sends `AvailableCommandsUpdate` on session creation

---

**Issue**: Commands not updating after agent change

**Possible Causes**:
- Session not switched properly
- Commands not loaded for new session

**Solution**:
- Ensure `on_agent_changed()` is called
- Check that `current_session_id` is updated
- Verify new session has commands available
