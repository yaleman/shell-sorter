# Shell Sorter development tasks

# Run linting checks
lint:
    uv run ruff check shell_sorter/

# Run type checking
mypy:
    uv run mypy --strict shell_sorter/

# Run all checks (lint then mypy)
check: lint mypy

# Format code
fmt:
    uv run ruff fmt shell_sorter/

# Run the application
run:
    uv run shell-sorter