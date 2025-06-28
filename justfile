# Shell Sorter development tasks

# Run linting checks
lint:
    ruff check shell_sorter/

# Run type checking
mypy:
    mypy --strict shell_sorter/

# Run all checks (lint then mypy)
check: lint mypy

# Format code
fmt:
    ruff fmt shell_sorter/

# Run the application
run:
    python -m shell_sorter.app