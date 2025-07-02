# Shell Sorter development tasks

# list the available tasks
default:
    just --list

# Run linting checks
lint:
    uv run ruff check shell_sorter/ tests
    uv run ruff format --check shell_sorter/ tests
    # uv run pylint tests shell_sorter/

# Run type checking
mypy:
    uv run mypy --strict shell_sorter/

# Run tests
test:
    uv run pytest -s -v

# Run all checks
check: lint mypy test esphome-check

# Format code
fmt:
    uv run ruff fmt shell_sorter/

# Run the application
run:
    killall shell-sorter || true
    uv run shell-sorter

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
    yamllint esphome-shell-sorter.yaml
    yamllint esphome-esp32cam1.yaml
    esphome config esphome-shell-sorter.yaml > /dev/null
    esphome config esphome-esp32cam1.yaml > /dev/null