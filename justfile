# Shell Sorter development tasks

# list the available tasks
default:
    just --list

# Run linting checks
lint:
    cargo clippy --all-targets
    uv run ruff check shell_sorter/ tests

# Run type checking
mypy:
    uv run mypy --strict shell_sorter/

# Run tests
test:
    uv run pytest -s -v
    cargo test

# Run all checks
check: fmt lint mypy test esphome-check

# Format code
fmt:
    uv run ruff format --check shell_sorter/ tests
    cargo fmt

# Run the application
run:
    killall shell-sorter || true
    uv run shell-sorter

rust:
    cargo run

# Flash ESPHome configuration to device
esphome-flash:
    esphome upload esphome-shell-sorter.yaml

# flash and monitor the device
esphome-monitor:
    esphome run esphome-shell-sorter.yaml

# Build ESPHome configuration for shell sorter
esphome-cam-build:
    esphome compile esphome-esp32cam1.yaml

# Flash ESP32 Camera configuration to device
esphome-cam-flash: esphome-cam-build
    esphome upload esphome-esp32cam1.yaml

# flash and monitor the ESP32 camera device
esphome-cam-monitor:
    esphome run esphome-esp32cam1.yaml

esphome-check:
    yamllint esphome*.yaml
    esphome config esphome-shell-sorter.yaml > /dev/null
    esphome config esphome-esp32cam1.yaml > /dev/null