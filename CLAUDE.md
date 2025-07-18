## Project Overview

This application controls an ammunition shell case sorting machine that uses
computer vision and machine learning to automatically identify and sort
different types of shell cases.

## Development Guidelines

- It is mandatory that 'just check' finishes without warnings or errors before
  considering a task complete
- It is mandatory that the final step in completing a task is that all changes
  are commited to git
- Any time the design or implementation changes, CLAUDE.md must be updated
- It is mandatory that README.md is kept up to date with system design, hardware
  requirements, and setup instructions
- Never allow a bare "type: ignore" comment
- Never use global variables
- Never use inline javascript or css in a web page unless there's no other way
  to solve the problem
- If you're thinking about implementing backwards compatibility, check with the
  user first
- Never use `std::env::set_var`
- Unsafe code is a last resort, ask the user before continuing if that's the
  solution
- Use the tracing crate for logging in Rust, with the debug CLI flag enabling
  debug logging, and the default logging level set to "info"
- If you use unwrap or expect in production code, you have failed and will be
  terminated.
- Never use the 'timeout' command

## Architecture

[Rest of the file remains unchanged]