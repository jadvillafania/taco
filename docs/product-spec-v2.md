# Developer Visual Companion
## Product Specification — MVP

**Status:** Draft — rev 2 (message delivery strategy decided)  
**Product Type:** Local desktop utility  
**Primary Platform:** Windows + WSL2  
**Primary Agent:** Claude Code  
**Architecture Principle:** Local-first, no AI model required

---

## 1. Product Overview

Developer Visual Companion is a lightweight desktop application that allows software developers to capture visual information from their desktop and send it directly into an existing AI coding-agent session.

The primary use case is developers running Claude Code inside WSL. Claude Code provides an excellent coding-agent workflow, but interacting with visual information is less convenient than using an AI desktop application that supports image attachments.

The application bridges that gap:

```text
Developer
    │
    │ sees something
    ↓
Hotkey
    │
    ↓
Capture screen region
    │
    ↓
Optional instruction
    │
    ↓
Developer Visual Companion
    │
    ↓
Existing Claude Code session
    │
    ↓
Claude analyzes image
    │
    ↓
Continues existing development task
```

The application does **not** attempt to replace Claude Code, provide its own AI chat, or maintain an independent conversation.

Its job is to make visual input available to the agent the developer is already using.

---

# 2. Problem

A developer may already have an active Claude Code conversation:

```text
~/projects/my-app

$ claude

> Implement the new inventory interface.

Claude is currently working on the task...
```

While working, the developer encounters a visual problem:

- a UI element is misaligned
- a browser error appears
- a design reference needs to be implemented
- a terminal error needs to be shown
- a diagram needs to be explained
- a third-party documentation page needs to be interpreted
- a visual regression needs investigation

With a desktop AI application, the developer can simply attach a screenshot.

With Claude Code running inside WSL, the workflow is much less convenient.

The developer may have to:

1. Take a screenshot.
2. Save the image.
3. Find the file.
4. Figure out how to make the file available to WSL.
5. Manually communicate the path.
6. Potentially leave the existing workflow.
7. Provide additional context manually.

This creates unnecessary friction.

---

# 3. Product Goal

Make sending visual context to an active Claude Code session feel as natural as copying and pasting text.

The ideal workflow is:

```text
See something
    ↓
Hotkey
    ↓
Select area
    ↓
Optional explanation
    ↓
Send
    ↓
Claude sees it
```

The entire operation should take only a few seconds.

---

# 4. Product Principles

## 4.1 Claude Code remains the AI

The product should not introduce another AI conversation.

Claude Code remains responsible for:

- reasoning
- code analysis
- repository inspection
- implementation
- terminal commands
- testing
- modifications

The companion only provides additional context.

---

## 4.2 Preserve the existing conversation

The developer should not have to start another conversation.

If Claude Code is currently discussing:

```text
Implement the inventory page
```

the screenshot should become the next message in that same conversation.

This preserves:

- repository context
- previous instructions
- architectural decisions
- conversation history
- current task
- agent state

---

## 4.3 Local-first

The application should not require a backend for MVP.

Screenshots should remain on the developer's machine unless the developer explicitly sends them through an external service.

No account should be required.

No application-specific cloud storage should be required.

---

## 4.4 Minimal UI

The companion should stay out of the developer's way.

The primary interface should be:

- global hotkey
- screen capture overlay
- small capture preview
- send action
- system tray menu

It should not become another large application window.

---

# 5. Target User

Primary user:

> Software developer using Claude Code inside WSL on Windows.

Typical environment:

```text
Windows
├── VS Code
├── Browser
├── Terminal
│   └── WSL
│       └── Claude Code
└── Developer Visual Companion
```

Secondary future users:

- developers using Codex CLI
- developers using Gemini CLI
- developers using other terminal AI agents
- developers using remote development environments

---

# 6. MVP Scope

## Included

### Screenshot capture

- Region capture
- Full-screen capture
- Active-window capture

### Input

- Optional text instruction
- Clipboard image capture
- Clipboard text capture

### Claude Code integration

- Detect active Claude Code sessions
- Identify project/session
- Send visual context to selected session
- Continue existing conversation

### Desktop integration

- Global hotkey
- System tray application
- Windows startup option
- WSL integration

### Session management

Display currently detected sessions:

```text
Claude Code Sessions

● my-app
  ~/projects/my-app

○ backend
  ~/projects/backend

○ website
  ~/projects/company-web
```

---

# 7. Core User Experience

## 7.1 Capture Region

Developer presses:

```text
Ctrl + Shift + Space
```

The screen becomes dimmed.

The developer drags over the desired region.

```text
┌─────────────────────────────────────┐
│                                     │
│    ┌───────────────────────────┐    │
│    │                           │    │
│    │      Selected Region      │    │
│    │                           │    │
│    └───────────────────────────┘    │
│                                     │
└─────────────────────────────────────┘
```

After releasing the mouse, the capture is stored locally.

---

# 8. Capture Composer

A small floating window appears near the selected region.

```text
┌──────────────────────────────────────┐
│ Screenshot                           │
│                                      │
│ ┌──────────────────────────────────┐ │
│ │                                  │ │
│ │          Screenshot              │ │
│ │                                  │ │
│ └──────────────────────────────────┘ │
│                                      │
│ Message                              │
│ ┌──────────────────────────────────┐ │
│ │ Why is this UI misaligned?       │ │
│ └──────────────────────────────────┘ │
│                                      │
│ Claude Code: my-app                  │
│                                      │
│              [ Cancel ] [ Send ]     │
└──────────────────────────────────────┘
```

The message is optional.

If no message is supplied, the application should still be able to send the screenshot.

---

# 9. Quick Actions

The composer may provide optional predefined actions.

```text
[ Explain ] [ Debug ] [ Implement ]

[ Find source ] [ Review ]
```

These are not AI functionality.

They simply insert predefined instructions.

Example:

### Explain

```text
Explain what is shown in this screenshot and how it relates to the current application.
```

### Debug

```text
Analyze this screenshot and determine what appears to be wrong. Inspect the relevant code and identify the likely cause. Do not modify anything yet.
```

### Implement

```text
Use this screenshot as visual reference and implement the required changes in the current project.
```

### Find Source

```text
Identify which component, page, or code is responsible for the UI shown in this screenshot.
```

---

# 10. Annotation

Optional annotation should be supported after capture.

Tools:

```text
Pen
Arrow
Rectangle
Circle
Text
Blur
Undo
```

Example:

```text
┌───────────────────────────────┐
│                               │
│       [ Submit ]              │
│           ↑                   │
│       ┌────────┐              │
│       │ WRONG  │              │
│       └────────┘              │
│                               │
└───────────────────────────────┘
```

Annotations are permanently composited into the image before sending.

---

# 11. Clipboard Workflow

The application should support copying an image and sending it directly.

Example:

```text
Browser
    ↓
Copy Image
    ↓
Ctrl + Shift + V
    ↓
Developer Visual Companion
    ↓
Claude Code
```

If the clipboard contains an image, the application should recognize it automatically.

If the clipboard contains text, it may optionally be sent as text context.

---

# 12. Session Detection

The application should detect running Claude Code sessions.

A session should expose:

```text
Session ID
Process ID
Terminal
WSL distribution
Working directory
Project name
Session status
```

Example:

```text
Claude Code

● my-app
  Ubuntu
  ~/projects/my-app
  Active

○ api
  Ubuntu
  ~/projects/api
  Waiting
```

Detection sources, in order of authority:

```text
1. Shim registry
   sessions launched via dvc-shim
   → Tier 1 delivery capable

2. Hook registration
   SessionStart / SessionEnd hooks append and
   remove entries in a registry file
   → detection only

3. Process scan
   read-only heuristic over running `claude`
   processes and recent session activity
   → detection only
```

Sessions found only via (2) or (3) are still listed, but receive
Tier 2 (clipboard-assist) delivery and an inline hint offering to
enable the shim wrapper.

The application should associate the currently focused development environment with the most likely session.

---

# 13. Automatic Session Selection

The preferred behavior is:

```text
Developer is working in:

VS Code
~/projects/my-app

        ↓

Companion detects:

Claude Code
~/projects/my-app

        ↓

Automatically select:

● my-app
```

The developer should not need to manually select the session in normal usage.

A manual session selector remains available as a fallback.

---

# 14. Claude Code Integration

The most important technical requirement is reliable image/context injection into an already-running Claude Code session.

The integration layer should be abstracted:

```text
Companion Core
      │
      ↓
Agent Integration Interface
      │
      ├── Claude Code
      ├── Codex CLI
      ├── Gemini CLI
      └── Future agents
```

MVP only implements:

```text
ClaudeCodeAdapter
```

The adapter is responsible for:

- detecting sessions
- identifying the target session
- selecting the delivery tier (Section 15)
- transferring captured images
- transferring accompanying text
- injecting the input into the existing conversation
- reporting success/failure

---

# 15. Message Delivery

The delivery strategy is decided. It is tiered and evaluated per send:

```text
Tier 1   PTY Shim           instant injection (golden path)
Tier 2   Clipboard-Assist   universal fallback
Tier 3   Hooks Inbox        deferred delivery (optional)
```

In every tier, the image itself travels the same way:

```text
Screenshot
    ↓
Temporary PNG
%LOCALAPPDATA%/DeveloperVisualCompanion/captures/...
    ↓
WSL-visible path
/mnt/c/Users/<user>/AppData/Local/DeveloperVisualCompanion/captures/...
    ↓
Path referenced in a text message to Claude Code
```

File paths are the only image transport. Claude Code reads image files
from disk reliably in every terminal; clipboard **image** paste inside
WSL is not dependable and is not used.

---

## 15.1 Tier 1 — PTY Shim (primary)

A small self-contained Linux binary is deployed into the WSL
distribution by the companion itself. No package manager. No user
installation steps. Installed and removed only with explicit consent.

Sessions started through the shim wrapper:

```text
Terminal
   ↓
dvc-shim
   │  owns the PTY
   │  relays all I/O transparently
   │  listens on a Unix socket
   │  registers the session
   ↓
claude
```

Send path:

```text
Companion
   ↓
WSL Bridge
   ↓
Unix socket
   ↓
Shim writes message into Claude Code stdin
   ↓
Message appears in the session as if typed
```

Properties:

- instant delivery into the existing conversation
- no focus stealing
- terminal-emulator independent
- provides authoritative session detection (see Section 12)

Limitation:

- only covers sessions launched through the wrapper

---

## 15.2 Tier 2 — Clipboard-Assist (guaranteed fallback)

Used whenever the target session is not shim-managed.

```text
Save PNG
   ↓
Copy WSL path + message to clipboard
   ↓
Notification:
"Ready — paste into your Claude Code terminal"
   ↓
Optional: focus the target terminal window
```

Works in every terminal and every configuration on day one.

Every other tier degrades to this. A send must never hard-fail while
Tier 2 is possible.

---

## 15.3 Tier 3 — Hooks Inbox (optional enhancer)

A `UserPromptSubmit` hook — a single settings entry written by the
companion, with consent — checks a pending-captures queue and attaches
queued screenshots to the developer's next prompt.

```text
Capture
   ↓
Queue
   ↓
Developer submits next prompt
   ↓
Hook appends queued captures
```

Semantics are deferred, not immediate. The UI must label this clearly:

```text
Queued — will attach to your next Claude message
```

Off by default. Never the flagship interaction.

---

## 15.4 Rejected approaches

Evaluated and rejected. Do not revisit for MVP.

```text
Windows console input injection
(AttachConsole / WriteConsoleInput / SendInput)
→ terminal-emulator dependent
→ focus stealing and typing races

Headless sidecar
(claude -p --resume <session-id>)
→ response invisible in the developer's TUI
→ risk of forked session state

Clipboard image injection into WSL
→ WSLg clipboard bridge unreliable
```

If Claude Code later ships an official injection API, the Agent
Adapter (Section 28) absorbs it as a new Tier 1 implementation with no
product-level changes.

---

## 15.5 Default instruction

If the developer supplies no message, a default instruction accompanies
the path:

```text
Analyze this screenshot in the context of the current task.
```


---

# 16. Temporary Files

Captured images should use temporary storage.

Example:

```text
%LOCALAPPDATA%/
    DeveloperVisualCompanion/
        captures/
            2026-08-17/
                capture-193421.png
                capture-193455.png
```

Files should have configurable retention.

Default:

```text
Delete after 24 hours
```

Optional:

```text
Keep until manually deleted
```

---

# 17. Privacy

Screenshots can contain highly sensitive information.

The application must clearly communicate that:

> Screenshots are captured locally and are only sent to the selected AI agent when the developer explicitly presses Send.

The application should not:

- upload screenshots automatically
- maintain a remote screenshot database
- perform background OCR by default
- send telemetry containing screenshot contents

---

# 18. Security

The companion should avoid storing:

- credentials
- access tokens
- passwords
- cookies
- private keys

Screenshots should be treated as sensitive temporary data.

The application should provide:

```text
Delete capture
Clear capture history
Clear all temporary data
```

A future version may provide automatic sensitive-region detection/redaction.

---

# 19. System Tray

The application should primarily run from the Windows system tray.

Example:

```text
Developer Visual Companion

● Claude Code: Connected
  my-app

───────────────

Capture Region       Ctrl+Shift+Space
Capture Screen
Clipboard → Claude

Claude Sessions

Settings

Exit
```

---

# 20. Settings

Settings should remain minimal.

### Capture

- Region capture hotkey
- Full-screen hotkey
- Clipboard hotkey
- Default image format
- Image quality

### Claude Code

- Auto-detect sessions
- Default session
- Session selection behavior

### Storage

- Temporary file location
- Retention period

### Behavior

- Start with Windows
- Show capture composer
- Automatically send after capture
- Play notification sound
- Show success notification

---

# 21. Notifications

After successful transmission:

```text
✓ Screenshot sent to Claude Code
my-app
```

On failure:

```text
⚠ Could not send screenshot

Claude Code session is no longer available.

[Retry] [Select Session]
```

Notifications should be short and non-intrusive.

---

# 22. Error Handling

The application must handle:

### No Claude Code session

```text
No Claude Code session detected.

Open Claude Code and try again.
```

### Unmanaged session (no shim)

Deliver via Tier 2 (clipboard-assist) and offer to enable the shim
wrapper for instant delivery next time.

### Multiple sessions

Show session selector.

### Session disappeared

Allow retry or select another session.

### WSL unavailable

```text
WSL connection unavailable.
```

### Image transfer failed

Preserve the capture and provide retry.

The screenshot should not be lost merely because Claude Code is unavailable.

---

# 23. Performance Requirements

The companion should feel instantaneous.

Target:

```text
Hotkey → capture overlay
< 100 ms

Capture → preview
< 300 ms

Send → transfer begins
< 500 ms
```

The application should consume minimal resources while idle.

Target idle usage:

```text
CPU: negligible
RAM: <100 MB
Network: none
```

---

# 24. Offline Behavior

Screenshot capture must work without internet access.

The application itself does not require cloud connectivity.

Only the AI agent may require network access.

If Claude Code is unavailable:

```text
Capture
   ↓
Store locally
   ↓
Retry later
```

---

# 25. Architecture

Recommended architecture:

```text
┌─────────────────────────────────────────┐
│             Windows Desktop             │
│                                         │
│  ┌─────────────┐     ┌──────────────┐  │
│  │ Global      │     │ Screen       │  │
│  │ Hotkeys     │     │ Capture      │  │
│  └──────┬──────┘     └──────┬───────┘  │
│         │                   │          │
│         └─────────┬─────────┘          │
│                   ↓                    │
│          ┌────────────────┐            │
│          │ Capture Manager│            │
│          └───────┬────────┘            │
│                  ↓                     │
│          ┌────────────────┐            │
│          │ Composer /     │            │
│          │ Annotation     │            │
│          └───────┬────────┘            │
│                  ↓                     │
│          ┌────────────────┐            │
│          │ Session Manager│            │
│          └───────┬────────┘            │
│                  ↓                     │
│          ┌────────────────┐            │
│          │ Agent Adapter  │            │
│          │ Claude Code    │            │
│          └───────┬────────┘            │
└──────────────────┼─────────────────────┘
                   ↓
             ┌───────────┐
             │    WSL    │
             │           │
             │ Claude    │
             │ Code      │
             └───────────┘
```

---

# 26. Recommended MVP Technology

The application is fundamentally a Windows desktop utility.

Potential implementation choices:

### Tauri

Recommended candidate.

Advantages:

- lightweight
- native system integration
- Windows support
- tray support
- global shortcuts
- web-based UI
- Rust backend
- relatively small memory footprint

Frontend:

```text
Vue + TypeScript
```

Backend:

```text
Rust
```

This is particularly attractive for a developer-oriented utility where low resource usage matters.

Alternative:

```text
Electron
```

Electron would provide easier JavaScript ecosystem integration but would have a larger runtime footprint.

---

# 27. WSL Bridge

The WSL integration is isolated into a dedicated module.

```text
Windows Companion
       │
       ↓
WSL Bridge
       │
       ├── Shim Deployer
       ├── Registry Reader
       └── Socket Client
       │
       ↓
WSL Distribution
       │
       ↓
dvc-shim ──▶ Claude Code
```

Bridge interface:

```text
detectWsl()
listDistributions()
deployShim(distro)
removeShim(distro)
readRegistry(distro)
sendToSession(session, payload)
```

---

## 27.1 Shim deployment

```text
Static musl-linked binary
No runtime dependencies
Copied via \wsl$\<distro>\... or wsl.exe
Location: ~/.local/share/dvc/dvc-shim
```

Wrapper installation (explicit consent, reversible in Settings):

```text
alias claude='~/.local/share/dvc/dvc-shim claude'
```

---

## 27.2 Shim behavior

```text
1. Spawn `claude` inside a PTY it owns
2. Relay all I/O transparently
   (raw mode, window-resize passthrough)
3. Listen on a Unix socket
   $XDG_RUNTIME_DIR/dvc/<session>.sock
4. Register the session:
   { pid, cwd, distro, project, socket, started_at }
5. Deregister and remove the socket on exit
```

The shim must be invisible: identical TUI behavior, scrollback,
colors, and shortcuts as running `claude` directly.

---

## 27.3 Socket protocol

Line-delimited JSON:

```text
→ { "type": "send",  "id": "…", "text": "…" }
← { "type": "ack",   "id": "…" }
← { "type": "error", "id": "…", "reason": "…" }
```

An `ack` is required before the companion reports success
(Section 21). Anything else triggers retry or Tier 2 fallback.

---

## 27.4 Injection semantics

```text
1. Write message text literally to stdin
2. Short delay
3. Write Enter separately
```

Text and Enter are sent separately to avoid multiline/paste edge cases
in the TUI.

If the developer is actively typing, injected text could interleave
with their partial input. The shim should wait for a short input-idle
window before injecting; if the idle window is not reached within a
timeout, the companion falls back to Tier 2 rather than corrupting the
prompt.


---

# 28. Agent Adapter Interface

Conceptually:

```text
interface AgentAdapter {
    detectSessions(): Session[]
    isAvailable(): boolean
    send(
        session: Session,
        image: Image,
        message?: string
    ): SendResult
}
```

Claude implementation:

```text
ClaudeCodeAdapter
```

Future implementations:

```text
CodexAdapter
GeminiAdapter
```

This keeps the product from becoming permanently coupled to Claude Code.

---

# 29. MVP User Stories

### Capture

> As a developer, I want to capture part of my screen using a global hotkey so I don't have to manually save screenshots.

### Send

> As a developer, I want to send the captured image to my active Claude Code session without starting another conversation.

### Context

> As a developer, I want to add a short explanation to my screenshot so Claude understands what I want it to investigate.

### Session selection

> As a developer, I want the application to automatically select the Claude Code session associated with my current project.

### Retry

> As a developer, I want a failed screenshot transmission to remain available so I can retry it.

### Clipboard

> As a developer, I want to send an image copied from another application directly to Claude Code.

---

# 30. Explicit Non-Goals

MVP will **not** include:

- AI model hosting
- built-in chatbot
- project management
- code editing
- repository browsing
- RAG
- cloud storage
- team collaboration
- centralized administration
- analytics
- screenshot sharing
- automatic screenshot interpretation
- automatic OCR
- autonomous coding

These features distract from the core problem.

---

# 31. Future Features

## 31.1 Multiple agents

```text
Claude Code
Codex
Gemini CLI
OpenCode
```

---

## 31.2 Smart project detection

Automatically associate:

```text
VS Code workspace
    ↓
Git repository
    ↓
Claude Code session
```

---

## 31.3 Screenshot history

```text
Recent Captures

19:34  Login UI
19:29  API error
19:18  Dashboard
18:52  Database diagram
```

Allow:

```text
Resend
Delete
Open
Copy
```

---

## 31.4 Multi-image conversations

Capture several regions:

```text
Screenshot 1
Screenshot 2
Screenshot 3
       ↓
      Claude
```

Useful when explaining complex UI behavior.

---

## 31.5 Visual comparison

Capture:

```text
Expected UI
Actual UI
```

Then send both together.

Example instruction:

```text
The first image is the expected design.
The second image is the current implementation.
Identify the differences and implement the necessary changes.
```

This could become a particularly powerful workflow for frontend developers.

---

## 31.6 Screen recording

Future:

```text
Ctrl + Shift + R
```

Record a short interaction.

Example:

```text
Developer clicks button
↓
Modal opens
↓
UI breaks
↓
Recording sent to agent
```

This could be significantly more useful than screenshots for reproducing UI bugs.

---

## 31.7 Sensitive information detection

Automatically identify likely:

- passwords
- API keys
- tokens
- email addresses
- credentials

and warn or redact before sending.

---

# 32. Success Criteria

The MVP succeeds if developers can perform this workflow:

```text
Working in VS Code
        ↓
See visual problem
        ↓
Press hotkey
        ↓
Select region
        ↓
Type optional instruction
        ↓
Send
        ↓
Claude Code receives visual context
        ↓
Claude continues existing conversation
```

without:

- switching to another AI application
- manually saving the screenshot
- manually finding the screenshot
- manually copying a path
- restarting the Claude conversation
- re-explaining the development context

The target experience is:

> **"I saw something → I showed it to Claude."**

Nothing more complicated than that.

---

# 33. Product Positioning

The product should not be positioned as:

> "Another AI coding assistant."

It should be positioned as:

> **"The missing visual input layer for your coding agent."**

Or more simply:

> **"See it. Capture it. Send it to your agent."**

The product's value comes from eliminating a small but extremely frequent source of friction in AI-assisted software development.

---

# 34. MVP Priority

### P0 — Must Have

- Windows desktop application
- system tray
- global hotkey
- region capture
- full-screen capture
- capture preview
- optional text instruction
- PTY shim + wrapper deployment (consented)
- shim session registry (detection)
- Tier 1 socket delivery
- Tier 2 clipboard-assist fallback
- WSL bridge
- retry on failure

### P1 — Should Have

- clipboard image
- active-window capture
- hooks inbox (Tier 3, deferred delivery)
- hook-based session registration
- annotation
- automatic session selection
- session selector
- capture history

### P2 — Later

- multiple AI agents
- screen recording
- visual comparison
- sensitive-information detection
- team functionality
- cloud synchronization

---

# 35. Core MVP Philosophy

The application should remain extremely small.

The developer already has:

```text
AI       → Claude Code
Editor   → VS Code
Terminal → WSL
Git      → GitHub
```

Do not replace any of them.

Add only the missing capability:

```text
                 ┌─────────────────────┐
                 │ Developer Visual     │
                 │ Companion            │
                 │                     │
Screen ─────────→│ Capture → Claude    │
Clipboard ──────→│                     │
                 └─────────────────────┘
```

The product should feel less like installing another application and more like **adding a new capability to Claude Code itself**.

That simplicity is the core of the product.