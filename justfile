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
    killall shell-sorter || true
    uv run shell-sorter

# Flash ESPHome configuration to device
esphome-flash DEVICE:
    esphome upload esphome-shell-sorter.yaml

# flash and monitor the device
esphome-monitor DEVICE:
    esphome run esphome-shell-sorter.yaml