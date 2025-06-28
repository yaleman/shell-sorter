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

# Start ESPHome dashboard in Docker
esphome:
    docker run --rm -v "$(pwd):/config" -p 6052:6052 esphome/esphome dashboard /config

# Flash ESPHome configuration to device
esphome-flash DEVICE:
    docker run --rm -v "$(pwd):/config" --device={{DEVICE}} esphome/esphome run /config/esphome-shell-sorter.yaml